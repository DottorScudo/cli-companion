//! Il ciclo di un canale di contatto: due attivita' nello stesso task, cosi'
//! fermare il task ferma tutto. La ricezione continua (`receive` con cursore, a
//! lungo polling, con il tetto `RECEIVE_TIMEOUT`) porta i messaggi in entrata; l'uscita legge la coda delle
//! notifiche dell'HUB (`notices`, limitata) e le manda ai collegamenti. Il
//! cursore si salva solo dopo aver trattato tutto il lotto (almeno una volta, con
//! dedupe per id esterno).

use crate::connectors::contracts::{ConnectorError, ReceiveBatch};
use crate::connectors::limits::{RECEIVE_TIMEOUT, ROUND_TIMEOUT};
use crate::connectors::runtime::contact::ContactCtx;
use crate::connectors::runtime::supervisor::{mark_stopped, save_state, Env, Pace};
use crate::connectors::runtime::time::with_timeout;
use crate::connectors::state::MAIN_CURSOR;
use crate::hub::ContactNotice;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;

/// Pausa dopo una ricezione senza messaggi: il long polling di un servizio vero
/// aspetta gia' da se', questa evita un ciclo a vuoto se risponde subito.
pub const IDLE_PAUSE: Duration = Duration::from_secs(1);

pub async fn run_contact(
    env: &Env,
    instance: &str,
    epoch: u64,
    cc: &ContactCtx<'_>,
    notices: Option<Receiver<ContactNotice>>,
    pace: &mut Pace<'_>,
) {
    let outgoing = async {
        if let Some(rx) = notices {
            if !run_notices(env, instance, epoch, cc, rx).await {
                return;
            }
        }
        std::future::pending::<()>().await;
    };
    tokio::select! {
        () = receive_loop(env, instance, cc, pace) => {}
        () = outgoing => {}
    }
}

/// Manda le notifiche finche' il canale e' aperto (`true` quando si chiude). Un
/// errore che richiede l'utente (credenziali) ferma il supervisore (`false`),
/// come in ricezione.
pub async fn run_notices(
    env: &Env,
    instance: &str,
    epoch: u64,
    cc: &ContactCtx<'_>,
    mut rx: Receiver<ContactNotice>,
) -> bool {
    while let Some(notice) = rx.recv().await {
        if let Err(error) = cc.send_notice(&notice).await {
            log::warn!("connettori {instance}: {error}");
            mark_stopped(env, instance, epoch, &error);
            return false;
        }
        save_state(env, instance, cc.state).await;
    }
    true
}

async fn handle_batch(cc: &ContactCtx<'_>, batch: &ReceiveBatch) -> Result<(), ConnectorError> {
    with_timeout(ROUND_TIMEOUT, async {
        for message in &batch.messages {
            cc.handle_inbound(message.clone()).await?;
        }
        Ok(())
    })
    .await
}

async fn receive_loop(env: &Env, instance: &str, cc: &ContactCtx<'_>, pace: &mut Pace<'_>) {
    loop {
        let cursor = cc.state.lock().cursor(MAIN_CURSOR).cloned();
        let received = with_timeout(RECEIVE_TIMEOUT, cc.connector.receive(cc.ctx, cursor)).await;
        let handled = match received {
            Ok(batch) => handle_batch(cc, &batch).await.map(|()| batch),
            Err(e) => Err(e),
        };
        match handled {
            Ok(batch) => {
                cc.state.lock().set_cursor(MAIN_CURSOR, batch.cursor);
                save_state(env, instance, cc.state).await;
                pace.success();
                if batch.messages.is_empty() && !pace.wait(IDLE_PAUSE).await {
                    return;
                }
            }
            Err(error) => {
                if !pace.failure(&error).await {
                    return;
                }
            }
        }
    }
}
