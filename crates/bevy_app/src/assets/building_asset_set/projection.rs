//! Offline authoring codec. Reuse the runtime serializer, including its f32
//! formatting, rather than maintaining a second canonical JSON implementation.

use serde::{Deserialize, Serialize};

use super::schema::*;
use super::validation::{canonical, digest, manifest_digest, require, validate_receipt};
use super::{BuildingAssetSetError, decode_buildingset};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    operation: String,
    manifest: String,
    receipt: Option<String>,
}

#[derive(Serialize)]
struct Response {
    manifest: String,
    receipt: Option<String>,
}

/// Canonicalization is not approval. Authoring must validate its independently
/// supplied approval/evidence before asking this codec to encode a release.
pub fn project_building_asset_json(input: &[u8]) -> Result<Vec<u8>, BuildingAssetSetError> {
    let request: Request = serde_json::from_slice(input)?;
    let (manifest, receipt) = match request.operation.as_str() {
        "seal" => {
            let mut manifest: BuildingAssetSetManifest = serde_json::from_str(&request.manifest)?;
            require(
                request.receipt.is_none() && manifest.receipt.is_none(),
                "seal input must not contain a receipt",
            )?;
            manifest.identity.manifest_sha256 = manifest_digest(&manifest)?;
            let receipt = if manifest.identity.authority == BuildingAssetAuthority::ReleaseApproved
            {
                let bytes = canonical(&BuildingPromotionReceipt {
                    schema_version: 1,
                    identity: manifest.identity.clone(),
                    art_approval_sha256: manifest.art_approval_sha256.clone().unwrap_or_default(),
                    decision: "release_approved".into(),
                    numeric_approval_sha256: manifest.numeric_approval_sha256.clone(),
                })?;
                let hash = digest(&bytes);
                manifest.receipt = Some(BuildingArtifact {
                    role: "authority:receipt".into(),
                    path: format!(
                        "building_sets/{}/{}/{hash}.json",
                        manifest.identity.kind.slug(),
                        manifest.identity.generation
                    ),
                    bytes: bytes.len() as u64,
                    sha256: hash,
                });
                Some(bytes)
            } else {
                None
            };
            (manifest, receipt)
        }
        "validate" => (
            decode_buildingset(request.manifest.as_bytes())?,
            request.receipt.map(String::into_bytes),
        ),
        _ => return Err(BuildingAssetSetError("unknown codec operation".into())),
    };
    let bytes = canonical(&manifest)?;
    decode_buildingset(&bytes)?;
    match (&manifest.receipt, &receipt) {
        (Some(record), Some(payload)) => {
            super::validation::validate_payload(record, payload)?;
            validate_receipt(&manifest, payload)?;
        }
        (None, None) => {}
        _ => return Err(BuildingAssetSetError("receipt presence differs".into())),
    }
    let utf8 = |bytes| String::from_utf8(bytes).expect("serde JSON emits UTF-8");
    Ok(serde_json::to_vec(&Response {
        manifest: utf8(bytes),
        receipt: receipt.map(utf8),
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_rejects_noncanonical_validation_and_mismatched_receipts() {
        let manifest = super::super::tests::fixture(
            BuildingAssetKind::Tank,
            BuildingAssetAuthority::ReleaseApproved,
        );
        let receipt = super::super::tests::receipt_bytes(&manifest);
        let request = |text: String, receipt: &[u8]| {
            serde_json::to_vec(&serde_json::json!({
                "operation": "validate", "manifest": text,
                "receipt": std::str::from_utf8(receipt).unwrap(),
            }))
            .unwrap()
        };
        assert!(
            project_building_asset_json(&request(
                serde_json::to_string_pretty(&manifest).unwrap(),
                &receipt
            ))
            .is_err()
        );
        let other = super::super::tests::fixture(
            BuildingAssetKind::MudMixer,
            BuildingAssetAuthority::ReleaseApproved,
        );
        assert!(
            project_building_asset_json(&request(
                String::from_utf8(canonical(&manifest).unwrap()).unwrap(),
                &super::super::tests::receipt_bytes(&other)
            ))
            .is_err()
        );
    }

    #[test]
    fn authoring_codec_uses_the_exact_runtime_identity_and_float_format() {
        for kind in BuildingAssetKind::ALL {
            for authority in [
                BuildingAssetAuthority::ArtPreview,
                BuildingAssetAuthority::IsolatedCandidate,
                BuildingAssetAuthority::ReleaseApproved,
            ] {
                let mut manifest = super::super::tests::fixture(kind, authority);
                if let Some(part) = manifest.parts.first_mut() {
                    part.translation_wu = [0.1, -0.0, 1.0e-7];
                }
                manifest.receipt = None;
                let response = project_building_asset_json(&serde_json::to_vec(&serde_json::json!({
                    "operation": "seal", "manifest": serde_json::to_string_pretty(&manifest).unwrap(),
                    "receipt": null,
                })).unwrap()).unwrap();
                let value: serde_json::Value = serde_json::from_slice(&response).unwrap();
                let sealed = value["manifest"].as_str().unwrap();
                let decoded = decode_buildingset(sealed.as_bytes()).unwrap();
                assert_eq!(decoded.identity.kind, kind);
                let checked = project_building_asset_json(
                    &serde_json::to_vec(&serde_json::json!({
                        "operation": "validate", "manifest": sealed, "receipt": value["receipt"],
                    }))
                    .unwrap(),
                )
                .unwrap();
                assert_eq!(response, checked);
            }
        }
    }
}
