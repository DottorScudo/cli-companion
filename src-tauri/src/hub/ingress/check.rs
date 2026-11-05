//! Controlli di una consegna prima di toccare disco o stato, puri: l'autore e'
//! `external` e l'origine e' la sua. La validazione completa dei campi
//! (id, nome, `external_id`) la fa `ChannelThreads::add_message` al salvataggio.

use super::{ExternalDelivery, ExternalWake};
use crate::hub::model::Author;
use crate::hub::text::bot_name;

pub fn check(delivery: &ExternalDelivery) -> Result<(), String> {
    let Author::External { connector, instance, .. } = &delivery.author else {
        return Err("l'autore di una consegna esterna deve essere un mittente esterno".to_string());
    };
    let origin = &delivery.origin;
    if &origin.connector != connector || &origin.instance != instance {
        return Err("l'origine non corrisponde al connettore e all'istanza dell'autore".to_string());
    }
    if let ExternalWake::Secretary(name) = &delivery.wake {
        bot_name(name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::DeliveryTarget;
    use super::*;
    use crate::hub::channel::Channel;
    use crate::hub::model::MessageOrigin;

    fn delivery(author: Author, wake: ExternalWake) -> ExternalDelivery {
        ExternalDelivery {
            channel: Channel::General,
            target: DeliveryTarget::Existing("thr-1".into()),
            author,
            body: "ciao".into(),
            files: Vec::new(),
            origin: MessageOrigin { connector: "email".into(), instance: "casa".into(), external_id: "1".into() },
            wake,
        }
    }

    fn external(connector: &str, instance: &str) -> Author {
        Author::External { connector: connector.into(), instance: instance.into(), name: "Mario".into() }
    }

    #[test]
    fn a_matching_external_author_passes() {
        assert!(check(&delivery(external("email", "casa"), ExternalWake::Secretary("ada".into()))).is_ok());
    }

    #[test]
    fn other_authors_are_rejected() {
        for author in [Author::User, Author::System, Author::Bot { name: "ada".into() }] {
            assert!(check(&delivery(author, ExternalWake::None)).is_err());
        }
    }

    #[test]
    fn the_origin_must_match_the_author() {
        assert!(check(&delivery(external("chat", "casa"), ExternalWake::None)).is_err());
        assert!(check(&delivery(external("email", "altra"), ExternalWake::None)).is_err());
    }

    #[test]
    fn a_secretary_to_wake_needs_a_valid_name() {
        assert!(check(&delivery(external("email", "casa"), ExternalWake::Secretary(" ".into()))).is_err());
    }
}
