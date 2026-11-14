use axum::extract::Multipart;
use std::path::Path;
use uuid::Uuid;
use vipayment_shared::AppError;

pub const MAX_PROOF_BYTES: usize = 2 * 1024 * 1024;

pub async fn save_payment_proof(
    mut multipart: Multipart,
    upload_dir: &str,
) -> Result<String, AppError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart tidak valid: {e}")))?
    {
        if field.name() != Some("file") {
            continue;
        }

        let declared_type = field.content_type().unwrap_or_default().to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("gagal membaca file: {e}")))?;

        if bytes.is_empty() {
            return Err(AppError::BadRequest("file bukti kosong".into()));
        }
        if bytes.len() > MAX_PROOF_BYTES {
            return Err(AppError::BadRequest(
                "ukuran bukti bayar maksimal 2 MB".into(),
            ));
        }

        let extension = detect_image_extension(&bytes, &declared_type).ok_or_else(|| {
            AppError::BadRequest("bukti bayar harus berupa gambar JPG, PNG, atau WEBP".into())
        })?;

        tokio::fs::create_dir_all(upload_dir).await?;
        let filename = format!("{}.{extension}", Uuid::new_v4());
        let path = Path::new(upload_dir).join(&filename);
        tokio::fs::write(path, &bytes).await?;

        return Ok(filename);
    }

    Err(AppError::BadRequest(
        "field `file` tidak ditemukan pada multipart".into(),
    ))
}

fn detect_image_extension(bytes: &[u8], declared_type: &str) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("png");
    }
    if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }

    // Fall back to the declared type only when the magic bytes are inconclusive.
    match declared_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_image_signatures() {
        assert_eq!(
            detect_image_extension(&[0xFF, 0xD8, 0xFF, 0x00], "application/octet-stream"),
            Some("jpg")
        );
        assert_eq!(
            detect_image_extension(
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
                "application/octet-stream"
            ),
            Some("png")
        );
        assert_eq!(
            detect_image_extension(b"RIFF0000WEBPdata", "application/octet-stream"),
            Some("webp")
        );
        assert_eq!(detect_image_extension(b"not an image", "text/plain"), None);
    }
}
