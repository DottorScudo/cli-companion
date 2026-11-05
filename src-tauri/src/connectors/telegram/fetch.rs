//! Lo scarico di un allegato: `getFile` per il percorso, poi un GET sull'URL del file.
//! Il token sta nell'URL: gli errori sono fissi. Il tetto della risposta e' quello degli allegati.

use super::api;
use super::classify::error_of;
use super::files::parse_file_info;
use super::token::{bot_token, file_url};
use crate::connectors::contracts::{ConnectorContext, ConnectorError, RemoteFile};
use crate::connectors::limits::RESPONSE_MAX_FILES;
use crate::hub::attachments::IncomingFile;

pub async fn fetch_file(ctx: &ConnectorContext, file: &RemoteFile) -> Result<IncomingFile, ConnectorError> {
    let info = parse_file_info(&api::get_file(ctx, &file.id).await?)?;
    let url = file_url(&bot_token(ctx)?, &info.path);
    let response = ctx.http().clone().with_max_body(RESPONSE_MAX_FILES).get(&url).await?;
    if !response.is_success() {
        return Err(error_of(response.status, &response.body));
    }
    Ok(IncomingFile { name: file.name.clone(), bytes: response.body })
}

#[cfg(test)]
mod tests;
