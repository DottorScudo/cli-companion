//! Quali messaggi scaricare in un giro (pura): a turno una regola per volta, cosi' una
//! regola molto trafficata non affama le altre.

/// Al piu' `budget` coppie (regola, id): a giri, un id per regola, ciascuna dal piu' vecchio.
/// Il secondo valore dice se qualcosa e' rimasto fuori (il prossimo giro riparte subito).
pub fn plan_fetch(lists: &[(String, Vec<String>)], budget: usize) -> (Vec<(String, String)>, bool) {
    let mut plan: Vec<(String, String)> = Vec::new();
    let mut depth = 0usize;
    loop {
        let mut progressed = false;
        for (route, ids) in lists {
            if plan.len() >= budget {
                break;
            }
            if let Some(id) = ids.get(depth) {
                plan.push((route.clone(), id.clone()));
                progressed = true;
            }
        }
        if !progressed || plan.len() >= budget {
            break;
        }
        depth += 1;
    }
    let total: usize = lists.iter().map(|(_, ids)| ids.len()).sum();
    let more = total > plan.len();
    (plan, more)
}

#[cfg(test)]
mod tests;
