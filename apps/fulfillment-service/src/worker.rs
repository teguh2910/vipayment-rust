use crate::order_client::{ClaimedOrder, OrderClient};
use vipayment_shared::provider::Provider;
use vipayment_shared::AppError;

/// One fulfillment iteration: submit at most one claimed order, then refresh
/// the status of a handful of in-flight ones.
pub async fn run_once(client: &OrderClient, provider: &Provider) -> Result<(), AppError> {
    submit_next(client, provider).await?;
    poll_processing(client, provider).await?;
    Ok(())
}

async fn submit_next(client: &OrderClient, provider: &Provider) -> Result<(), AppError> {
    let Some(order) = client.claim().await? else {
        return Ok(());
    };

    match submit_order(provider, &order).await {
        Ok((trxid, provider_status)) => {
            client
                .mark_submitted(&order.invoice, trxid.as_deref(), provider_status.as_deref())
                .await?;
        }
        // The provider refused the order: it must be refunded, not retried.
        Err(AppError::ProviderRejected(message)) => {
            client.mark_rejected(&order.invoice, &message).await?;
        }
        Err(error) => {
            client
                .mark_failed(&order.invoice, &error.to_string())
                .await?;
        }
    }

    Ok(())
}

/// Routes the order to the endpoint that matches its kind. Prepaid products are
/// sent to `/prepaid` and never carry a zone.
async fn submit_order(
    provider: &Provider,
    order: &ClaimedOrder,
) -> Result<(Option<String>, Option<String>), AppError> {
    if order.kind == "prepaid" {
        let data = provider
            .create_prepaid_order(&order.product_code, &order.user_id)
            .await?;
        Ok((data.trxid, data.status))
    } else {
        let data = provider
            .create_game_order(&order.product_code, &order.user_id, order.zone.as_deref())
            .await?;
        Ok((data.trxid, data.status))
    }
}

async fn poll_processing(client: &OrderClient, provider: &Provider) -> Result<(), AppError> {
    for order in client.pollable().await? {
        let Some(trxid) = order.provider_trxid.as_deref() else {
            continue;
        };

        let status_result = if order.kind == "prepaid" {
            provider.prepaid_order_status(trxid).await
        } else {
            provider.game_order_status(trxid).await
        };

        let item = match status_result {
            Ok(item) => item,
            Err(error) => {
                tracing::warn!(invoice = %order.invoice, trxid, %error, "status check failed");
                continue;
            }
        };

        let Some(item) = item else {
            continue;
        };

        let raw_status = item.status.clone().unwrap_or_default();
        if raw_status.is_empty() {
            continue;
        }

        // The order service maps the raw provider status to an order status.
        client
            .report_status(
                &order.invoice,
                &raw_status,
                item.sn.as_deref(),
                item.note.as_deref(),
            )
            .await?;
    }

    Ok(())
}
