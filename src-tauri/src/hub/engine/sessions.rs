//! Le sessioni dei segretari e le code dei thread (puro, tempo passato dal
//! chiamante). Una sessione e' la conversazione interna di un segretario su un
//! thread: si apre con una menzione, resta finche' passano `idle` millisecondi
//! senza attivita', poi si chiude con il riassunto del thread.
//!
//! Regole di risveglio (`route`):
//! - solo i messaggi dell'utente svegliano qualcuno (mai bot ne' sistema);
//! - le menzioni `@nome` di segretari configurati e ammessi nel canale
//!   svegliano quei segretari, in ordine; le altre producono un avviso;
//! - senza menzioni di segretari esistenti, il messaggio sveglia il primo
//!   segretario che puo' rispondere nel canale tra: l'autore del messaggio a
//!   cui l'utente risponde («Rispondi»), il titolare della sessione attiva piu'
//!   recente, il titolare persistito del thread (`Thread::owner`) e l'ultimo
//!   segretario che vi ha scritto (`Fallback`, ricavato dal chiamante). Cosi'
//!   anche un segretario addormentato (sessione scaduta, app riavviata, thread
//!   chiuso) si risveglia con una sessione nuova; se nessuno puo' rispondere,
//!   `Notice::Unanswered` chiede un avviso nel thread al posto del silenzio.
//!
//! Risvegli da evento (`route_event`): alcuni eventi della board svegliano il
//! segretario titolare del thread (`Thread::owner`, quello che ha aperto la
//! pianificazione; vedi `events::route`). Senza una sessione aperta ne parte
//! una nuova dall'evento; con una sessione aperta l'evento e' il suo turno
//! successivo (ha gia' il contesto, e due sessioni dello stesso segretario sullo
//! stesso thread non possono esistere). Un turno svegliato da un evento non puo'
//! decidere una proposta (`WakeCause`).
//!
//! Risvegli da elementi esterni (`route_external`): un connettore consegna un
//! elemento (un'email, un messaggio) e la regola di consegna sceglie il
//! segretario. Come per gli eventi, mai menzioni (un testo esterno con @mario
//! non sveglia nessuno: `route` ignora chi non e' l'utente) e il turno
//! (`WakeCause::External`) non puo' decidere proposte ne' rispondere ai worker.
//!
//! Ogni thread ha una coda (`Lane`) di lavori eseguiti uno alla volta, nello
//! stesso ordine: un messaggio che arriva mentre un segretario lavora aspetta
//! il suo turno. Il riassunto di una sessione scaduta passa dalla stessa coda,
//! cosi' una sessione nuova sullo stesso thread lo legge gia' aggiornato.

mod fallback;

pub use fallback::Fallback;

use super::chat::ChatMessage;
use super::mentions::mentions;
use crate::hub::channel::Channel;
use crate::hub::model::{Author, Message};
use crate::hub::settings::Secretary;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThreadKey {
    pub channel: Channel,
    pub thread_id: String,
}

impl ThreadKey {
    pub fn new(channel: &Channel, thread_id: &str) -> Self {
        Self { channel: channel.clone(), thread_id: thread_id.to_string() }
    }
}

#[derive(Debug, Clone)]
struct Session {
    secretary: String,
    last_activity: i64,
    /// Vuota fino al primo turno; presa dal turno in corso e poi restituita.
    conversation: Vec<ChatMessage>,
    /// Il messaggio che ha aperto l'ultimo turno: i turni dopo ricevono solo
    /// i messaggi successivi.
    last_seen: Option<String>,
}

/// Che cosa ha svegliato un turno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeCause {
    /// Un messaggio dell'utente (menzione o segretario scelto da `route`).
    Message,
    /// Un evento della board nel thread del segretario titolare.
    BoardEvent,
    /// Un elemento consegnato da una fonte esterna (un connettore): contenuto
    /// non fidato, mai una richiesta dell'utente.
    External,
}

/// Un lavoro della coda di un thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// Un turno del segretario per il messaggio `trigger` del thread.
    Reply { secretary: String, trigger: String, cause: WakeCause },
    /// Il riassunto del thread dalla conversazione di una sessione chiusa.
    Summarize { secretary: String, conversation: Vec<ChatMessage> },
}

impl Job {
    pub fn secretary(&self) -> &str {
        match self {
            Self::Reply { secretary, .. } | Self::Summarize { secretary, .. } => secretary,
        }
    }
}

#[derive(Debug, Default)]
struct Lane {
    queue: VecDeque<Job>,
    /// Il segretario del lavoro in corso.
    working: Option<String>,
    /// Un esecutore sta svuotando la coda.
    running: bool,
}

/// Perche' un messaggio dell'utente non sveglia un segretario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// Menzione di un segretario senza system prompt.
    NotConfigured(String),
    /// Menzione di un segretario fuori dai suoi canali.
    NotAllowed(String),
    /// Nessuna menzione e nessun candidato che possa rispondere.
    Unanswered,
}

/// L'esito di `route`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Routing {
    /// I segretari svegliati (o raggiunti nella loro sessione), in ordine.
    pub woken: Vec<String>,
    pub notices: Vec<Notice>,
    /// La coda del thread ha lavori e nessun esecutore: va avviato.
    pub start_lane: bool,
}

/// Cio' che un turno prende dalla sessione e poi restituisce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTurn {
    pub conversation: Vec<ChatMessage>,
    pub last_seen: Option<String>,
}

/// Un segretario sveglio, per lo stato verso il frontend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Awake {
    pub key: ThreadKey,
    pub secretary: String,
    pub expires_at: i64,
    pub working: bool,
}

/// Il segretario `name` dell'elenco, se puo' rispondere nel canale:
/// configurato e ammesso. Il motore lo ricontrolla a ogni turno, cosi' una
/// modifica delle impostazioni vale gia' dal turno successivo.
pub fn usable<'a>(secretaries: &'a [Secretary], name: &str, channel: &Channel) -> Option<&'a Secretary> {
    secretaries.iter().find(|s| s.name == name && s.is_configured() && s.channels.allows(channel))
}

#[derive(Debug, Default)]
pub struct Sessions {
    sessions: HashMap<ThreadKey, Vec<Session>>,
    lanes: HashMap<ThreadKey, Lane>,
}

impl Sessions {
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty() && self.lanes.is_empty()
    }

    /// Sessioni aperte, in tutti i thread.
    pub fn open_count(&self) -> usize {
        self.sessions.values().map(Vec::len).sum()
    }

    /// Decide chi sveglia il messaggio appena scritto nel thread `key`, apre o
    /// rinnova le sessioni e accoda i turni. `secretaries` = l'elenco corrente;
    /// `fallback` = i candidati del thread per un messaggio senza menzioni.
    pub fn route(
        &mut self,
        key: &ThreadKey,
        message: &Message,
        fallback: &Fallback,
        secretaries: &[Secretary],
        idle: i64,
        now: i64,
    ) -> Routing {
        let mut routing = Routing::default();
        if message.author != Author::User {
            return routing;
        }
        let mentioned: Vec<&Secretary> =
            mentions(&message.body).iter().filter_map(|name| secretaries.iter().find(|s| &s.name == name)).collect();
        if mentioned.is_empty() {
            match self.recipient(key, fallback, secretaries, idle, now) {
                Some(name) => routing.woken.push(name),
                None => routing.notices.push(Notice::Unanswered),
            }
        }
        for secretary in mentioned {
            if !secretary.is_configured() {
                routing.notices.push(Notice::NotConfigured(secretary.name.clone()));
            } else if !secretary.channels.allows(&key.channel) {
                routing.notices.push(Notice::NotAllowed(secretary.name.clone()));
            } else {
                routing.woken.push(secretary.name.clone());
            }
        }
        self.enqueue(key, &mut routing, &message.id, WakeCause::Message, idle, now);
        routing
    }

    /// Un evento della board (il messaggio `trigger`) sveglia il titolare
    /// `secretary` del thread, se puo' ancora rispondere nel canale: una
    /// sessione nuova, o il turno successivo di quella aperta.
    pub fn route_event(
        &mut self,
        key: &ThreadKey,
        secretary: &str,
        trigger: &str,
        secretaries: &[Secretary],
        idle: i64,
        now: i64,
    ) -> Routing {
        self.route_to(key, secretary, trigger, WakeCause::BoardEvent, secretaries, idle, now)
    }

    /// Un elemento esterno (il messaggio `trigger`) sveglia `secretary`, scelto
    /// dalla regola di consegna (titolare del thread o segretario nominato), se
    /// puo' rispondere nel canale. Mai menzioni: il testo esterno non decide
    /// chi si sveglia.
    pub fn route_external(
        &mut self,
        key: &ThreadKey,
        secretary: &str,
        trigger: &str,
        secretaries: &[Secretary],
        idle: i64,
        now: i64,
    ) -> Routing {
        self.route_to(key, secretary, trigger, WakeCause::External, secretaries, idle, now)
    }

    /// Il risveglio di un solo segretario, scelto da chi chiama e non dal testo.
    #[expect(clippy::too_many_arguments, reason = "tutto il contesto del risveglio, funzione pura")]
    fn route_to(
        &mut self,
        key: &ThreadKey,
        secretary: &str,
        trigger: &str,
        cause: WakeCause,
        secretaries: &[Secretary],
        idle: i64,
        now: i64,
    ) -> Routing {
        let mut routing = Routing::default();
        if usable(secretaries, secretary, &key.channel).is_some() {
            routing.woken.push(secretary.to_string());
        }
        self.enqueue(key, &mut routing, trigger, cause, idle, now);
        routing
    }

    /// Apre o rinnova le sessioni dei segretari svegliati e accoda i loro turni.
    fn enqueue(
        &mut self,
        key: &ThreadKey,
        routing: &mut Routing,
        trigger: &str,
        cause: WakeCause,
        idle: i64,
        now: i64,
    ) {
        for name in &routing.woken {
            self.touch(key, name, idle, now);
            let job = Job::Reply { secretary: name.clone(), trigger: trigger.to_string(), cause };
            self.lanes.entry(key.clone()).or_default().queue.push_back(job);
        }
        routing.start_lane = !routing.woken.is_empty() && self.claim_lane(key);
    }

    /// Chi riceve un messaggio senza menzioni: il primo candidato di
    /// `fallback` (con il titolare della sessione al suo posto) che puo'
    /// rispondere nel canale.
    fn recipient(
        &self,
        key: &ThreadKey,
        fallback: &Fallback,
        secretaries: &[Secretary],
        idle: i64,
        now: i64,
    ) -> Option<String> {
        let owner = self.owner(key, secretaries, idle, now);
        let recipient = fallback
            .candidates(owner.as_deref())
            .find(|name| usable(secretaries, name, &key.channel).is_some())
            .map(str::to_string);
        recipient
    }

    /// Il titolare della sessione: la sessione attiva piu' recente di un
    /// segretario che puo' ancora rispondere in quel canale.
    fn owner(&self, key: &ThreadKey, secretaries: &[Secretary], idle: i64, now: i64) -> Option<String> {
        self.sessions
            .get(key)?
            .iter()
            .filter(|s| !self.expired(key, s, idle, now) && usable(secretaries, &s.secretary, &key.channel).is_some())
            .max_by_key(|s| s.last_activity)
            .map(|s| s.secretary.clone())
    }

    /// Rinnova la sessione del segretario, o ne apre una nuova; una sessione
    /// scaduta (non ancora raccolta dallo schedulatore) si chiude prima, con
    /// il suo riassunto in coda.
    fn touch(&mut self, key: &ThreadKey, name: &str, idle: i64, now: i64) {
        let expired = self
            .sessions
            .get(key)
            .and_then(|list| list.iter().find(|s| s.secretary == name))
            .is_some_and(|s| self.expired(key, s, idle, now));
        if expired {
            self.close(key, name, true);
        }
        let list = self.sessions.entry(key.clone()).or_default();
        match list.iter_mut().find(|s| s.secretary == name) {
            Some(session) => session.last_activity = now,
            None => list.push(Session {
                secretary: name.to_string(),
                last_activity: now,
                conversation: Vec::new(),
                last_seen: None,
            }),
        }
    }

    /// Il prossimo lavoro della coda, che diventa quello in corso. Con la coda
    /// vuota l'esecutore si ferma: `None`, e la coda sparisce.
    pub fn next_job(&mut self, key: &ThreadKey) -> Option<Job> {
        let lane = self.lanes.get_mut(key)?;
        lane.working = None;
        match lane.queue.pop_front() {
            Some(job) => {
                lane.working = Some(job.secretary().to_string());
                Some(job)
            }
            None => {
                self.lanes.remove(key);
                None
            }
        }
    }

    /// Prende la conversazione della sessione per un turno; `None` se la
    /// sessione non c'e' piu' (HUB spento, segretario tolto).
    pub fn begin_turn(&mut self, key: &ThreadKey, name: &str) -> Option<SessionTurn> {
        let session = self.session_mut(key, name)?;
        Some(SessionTurn {
            conversation: std::mem::take(&mut session.conversation),
            last_seen: session.last_seen.clone(),
        })
    }

    /// Restituisce la conversazione dopo il turno: l'attivita' riparte da `now`.
    pub fn end_turn(&mut self, key: &ThreadKey, name: &str, turn: SessionTurn, now: i64) {
        if let Some(session) = self.session_mut(key, name) {
            session.conversation = turn.conversation;
            session.last_seen = turn.last_seen;
            session.last_activity = now;
        }
    }

    /// Il lavoro in corso sul thread e' finito.
    pub fn job_done(&mut self, key: &ThreadKey) {
        if let Some(lane) = self.lanes.get_mut(key) {
            lane.working = None;
        }
    }

    /// Chiude la sessione; con `summarize` (e una conversazione non vuota)
    /// accoda il riassunto e ritorna `true`: chi chiama avvia la coda se serve.
    pub fn close(&mut self, key: &ThreadKey, name: &str, summarize: bool) -> bool {
        let Some(list) = self.sessions.get_mut(key) else {
            return false;
        };
        let Some(at) = list.iter().position(|s| s.secretary == name) else {
            return false;
        };
        let session = list.remove(at);
        if list.is_empty() {
            self.sessions.remove(key);
        }
        if !summarize || session.conversation.is_empty() {
            return false;
        }
        let job = Job::Summarize { secretary: session.secretary, conversation: session.conversation };
        self.lanes.entry(key.clone()).or_default().queue.push_back(job);
        true
    }

    /// Chiude le sessioni scadute (non al lavoro ne' in coda) accodando i loro
    /// riassunti. Ritorna i thread la cui coda va avviata.
    pub fn expire_due(&mut self, idle: i64, now: i64) -> Vec<ThreadKey> {
        let due: Vec<(ThreadKey, String)> = self
            .sessions
            .iter()
            .flat_map(|(key, list)| list.iter().map(move |s| (key, s)))
            .filter(|(key, s)| self.expired(key, s, idle, now))
            .map(|(key, s)| (key.clone(), s.secretary.clone()))
            .collect();
        let mut start = Vec::new();
        for (key, name) in due {
            if self.close(&key, &name, true) && self.claim_lane(&key) {
                start.push(key);
            }
        }
        start
    }

    /// Quando scade la prossima sessione inattiva (`None` = nessuna).
    pub fn next_deadline(&self, idle: i64) -> Option<i64> {
        self.sessions
            .iter()
            .flat_map(|(key, list)| list.iter().filter(move |s| !self.busy(key, &s.secretary)))
            .map(|s| s.last_activity + idle)
            .min()
    }

    /// I segretari svegli, ordinati per canale, thread e nome.
    pub fn awake(&self, idle: i64) -> Vec<Awake> {
        let mut out: Vec<Awake> = self
            .sessions
            .iter()
            .flat_map(|(key, list)| {
                list.iter().map(move |s| Awake {
                    key: key.clone(),
                    secretary: s.secretary.clone(),
                    expires_at: s.last_activity + idle,
                    working: self.busy(key, &s.secretary),
                })
            })
            .collect();
        out.sort_by(|a, b| {
            (a.key.channel.as_str(), &a.key.thread_id, &a.secretary).cmp(&(
                b.key.channel.as_str(),
                &b.key.thread_id,
                &b.secretary,
            ))
        });
        out
    }

    /// Via sessioni e code del canale (il workspace e' stato rimosso).
    pub fn forget_channel(&mut self, channel: &Channel) {
        self.sessions.retain(|key, _| &key.channel != channel);
        self.lanes.retain(|key, _| &key.channel != channel);
    }

    /// Via tutto (HUB spento): nessun riassunto.
    pub fn clear(&mut self) {
        self.sessions.clear();
        self.lanes.clear();
    }

    /// Segna la coda come presa da un esecutore; `false` se lo era gia'.
    fn claim_lane(&mut self, key: &ThreadKey) -> bool {
        let lane = self.lanes.entry(key.clone()).or_default();
        !std::mem::replace(&mut lane.running, true)
    }

    fn session_mut(&mut self, key: &ThreadKey, name: &str) -> Option<&mut Session> {
        self.sessions.get_mut(key)?.iter_mut().find(|s| s.secretary == name)
    }

    /// Il segretario lavora sul thread o ha un turno in coda.
    fn busy(&self, key: &ThreadKey, name: &str) -> bool {
        self.lanes.get(key).is_some_and(|lane| {
            lane.working.as_deref() == Some(name)
                || lane.queue.iter().any(|job| matches!(job, Job::Reply { secretary, .. } if secretary == name))
        })
    }

    fn expired(&self, key: &ThreadKey, session: &Session, idle: i64, now: i64) -> bool {
        session.last_activity + idle <= now && !self.busy(key, &session.secretary)
    }
}

#[cfg(test)]
mod tests;
