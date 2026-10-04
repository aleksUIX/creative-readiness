//! JSON Schema catalog checks. Object schemas $ref defs.json via $id.

use jsonschema::{Retrieve, Uri};
use serde_json::Value;

struct CatalogRetriever;

impl Retrieve for CatalogRetriever {
    fn retrieve(
        &self,
        uri: &Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let base = uri.as_str().split('#').next().unwrap_or(uri.as_str());
        load_by_id(base).ok_or_else(|| format!("unknown schema {base}").into())
    }
}

fn load_by_id(id: &str) -> Option<Value> {
    let raw = match id {
        "https://creative-readiness.dev/schemas/defs.json" => {
            include_str!("../../../schemas/defs.json")
        }
        "https://creative-readiness.dev/schemas/readiness-evidence.json" => {
            include_str!("../../../schemas/readiness-evidence.json")
        }
        "https://creative-readiness.dev/schemas/policy-document.json" => {
            include_str!("../../../schemas/policy-document.json")
        }
        "https://creative-readiness.dev/schemas/creative-identity.json" => {
            include_str!("../../../schemas/creative-identity.json")
        }
        "https://creative-readiness.dev/schemas/prepared-media.json" => {
            include_str!("../../../schemas/prepared-media.json")
        }
        "https://creative-readiness.dev/schemas/observation.json" => {
            include_str!("../../../schemas/observation.json")
        }
        "https://creative-readiness.dev/schemas/eligibility.json" => {
            include_str!("../../../schemas/eligibility.json")
        }
        "https://creative-readiness.dev/schemas/bid-gate.json" => {
            include_str!("../../../schemas/bid-gate.json")
        }
        "https://creative-readiness.dev/schemas/readiness-summary.json" => {
            include_str!("../../../schemas/readiness-summary.json")
        }
        "https://creative-readiness.dev/schemas/capability-surface.json" => {
            include_str!("../../../schemas/capability-surface.json")
        }
        "https://creative-readiness.dev/schemas/delivery-environment.json" => {
            include_str!("../../../schemas/delivery-environment.json")
        }
        "https://creative-readiness.dev/schemas/validation-authority.json" => {
            include_str!("../../../schemas/validation-authority.json")
        }
        "https://creative-readiness.dev/schemas/operational-claims.json" => {
            include_str!("../../../schemas/operational-claims.json")
        }
        "https://creative-readiness.dev/schemas/opportunity.json" => {
            include_str!("../../../schemas/opportunity.json")
        }
        "https://creative-readiness.dev/schemas/policy-overlay.json" => {
            include_str!("../../../schemas/policy-overlay.json")
        }
        "https://creative-readiness.dev/schemas/transcode-ladder.json" => {
            include_str!("../../../schemas/transcode-ladder.json")
        }
        "https://creative-readiness.dev/schemas/auction-context.json" => {
            include_str!("../../../schemas/auction-context.json")
        }
        "https://creative-readiness.dev/schemas/environment-spec.json" => {
            include_str!("../../../schemas/environment-spec.json")
        }
        "https://creative-readiness.dev/schemas/environment-result.json" => {
            include_str!("../../../schemas/environment-result.json")
        }
        "https://creative-readiness.dev/schemas/diff-result.json" => {
            include_str!("../../../schemas/diff-result.json")
        }
        "https://creative-readiness.dev/schemas/finding.json" => {
            include_str!("../../../schemas/finding.json")
        }
        "https://creative-readiness.dev/schemas/media-facts.json" => {
            include_str!("../../../schemas/media-facts.json")
        }
        "https://creative-readiness.dev/schemas/openrtb-lite.json" => {
            include_str!("../../../schemas/openrtb-lite.json")
        }
        "https://creative-readiness.dev/schemas/evaluate-request.json" => {
            include_str!("../../../schemas/evaluate-request.json")
        }
        "https://creative-readiness.dev/schemas/authority-spec.json" => {
            include_str!("../../../schemas/authority-spec.json")
        }
        _ => return None,
    };
    Some(serde_json::from_str(raw).expect(id))
}

fn load_file(file: &str) -> Value {
    let id = format!("https://creative-readiness.dev/schemas/{file}");
    load_by_id(&id).unwrap_or_else(|| panic!("missing schema {file}"))
}

pub(crate) fn assert_schema(file: &str, instance: &Value) {
    let schema = load_file(file);
    let validator = jsonschema::options()
        .with_retriever(CatalogRetriever)
        .build(&schema)
        .unwrap_or_else(|e| panic!("schema {file} build: {e}"));
    if let Err(err) = validator.validate(instance) {
        panic!("{file}: {err}\ninstance: {instance}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diff_vast, evaluate, evaluate_paths, gate, module_surface, observe, prebid_gate, summarize,
        EnvironmentSpec, EvaluateRequest, Observation, OperationalClaims, Opportunity, Pack,
        PreparedMedia, SellerTreatment,
    };

    #[test]
    fn packs_and_overlays_match_schemas() {
        for (file, pack) in [
            (
                "policy-document.json",
                include_str!("../../../packs/live-ssai.json"),
            ),
            (
                "policy-document.json",
                include_str!("../../../packs/live-sgai.json"),
            ),
            (
                "policy-document.json",
                include_str!("../../../packs/live-csai.json"),
            ),
            (
                "policy-document.json",
                include_str!("../../../packs/live-hybrid.json"),
            ),
        ] {
            assert_schema(file, &serde_json::from_str(pack).unwrap());
        }
        for overlay in [
            include_str!("../../../fixtures/overlays/deal-premium.json"),
            include_str!("../../../fixtures/overlays/publisher-strict.json"),
            include_str!("../../../fixtures/overlays/event-sports.json"),
            include_str!("../../../fixtures/overlays/event-news.json"),
            include_str!("../../../fixtures/overlays/privacy-macros.json"),
            include_str!("../../../fixtures/overlays/audio-aac.json"),
            include_str!("../../../fixtures/overlays/device-ctv.json"),
            include_str!("../../../fixtures/overlays/closed-captions.json"),
            include_str!("../../../fixtures/overlays/caption-language.json"),
            include_str!("../../../fixtures/overlays/caption-type.json"),
            include_str!("../../../fixtures/overlays/id-registry.json"),
            include_str!("../../../fixtures/overlays/icon-program.json"),
            include_str!("../../../fixtures/overlays/ad-system-version.json"),
            include_str!("../../../fixtures/overlays/companion-width.json"),
            include_str!("../../../fixtures/overlays/companion-height.json"),
            include_str!("../../../fixtures/overlays/icon-position.json"),
            include_str!("../../../fixtures/overlays/ad-serving-id-length.json"),
            include_str!("../../../fixtures/overlays/vast-version.json"),
            include_str!("../../../fixtures/overlays/creative-sequence.json"),
            include_str!("../../../fixtures/overlays/max-expires.json"),
            include_str!("../../../fixtures/overlays/ad-sequence.json"),
            include_str!("../../../fixtures/overlays/wrapper-https.json"),
            include_str!("../../../fixtures/overlays/verification-vendor.json"),
            include_str!("../../../fixtures/overlays/pricing-model.json"),
            include_str!("../../../fixtures/overlays/pricing-currency.json"),
            include_str!("../../../fixtures/overlays/error-https.json"),
            include_str!("../../../fixtures/overlays/viewable-https.json"),
            include_str!("../../../fixtures/overlays/impression-https.json"),
            include_str!("../../../fixtures/overlays/clickthrough-https.json"),
            include_str!("../../../fixtures/overlays/survey-https.json"),
            include_str!("../../../fixtures/overlays/clicktracking-https.json"),
            include_str!("../../../fixtures/overlays/icon-clickthrough-https.json"),
            include_str!("../../../fixtures/overlays/category-authority.json"),
            include_str!("../../../fixtures/overlays/javascript-resource-https.json"),
            include_str!("../../../fixtures/overlays/companion-clickthrough-https.json"),
            include_str!("../../../fixtures/overlays/advertiser.json"),
            include_str!("../../../fixtures/overlays/executable-resource-https.json"),
            include_str!("../../../fixtures/overlays/nonlinear-clickthrough-https.json"),
            include_str!("../../../fixtures/overlays/description-length.json"),
            include_str!("../../../fixtures/overlays/static-resource-https.json"),
            include_str!("../../../fixtures/overlays/iframe-resource-https.json"),
            include_str!("../../../fixtures/overlays/ad-title-length.json"),
            include_str!("../../../fixtures/overlays/html-resource-https.json"),
            include_str!("../../../fixtures/overlays/flash-resource-https.json"),
            include_str!("../../../fixtures/overlays/ad-parameters-length.json"),
            include_str!("../../../fixtures/overlays/closed-caption-https.json"),
            include_str!("../../../fixtures/overlays/icon-viewtracking-https.json"),
            include_str!("../../../fixtures/overlays/ad-system.json"),
            include_str!("../../../fixtures/overlays/icon-clickfallback-https.json"),
            include_str!("../../../fixtures/overlays/mezzanine-https.json"),
            include_str!("../../../fixtures/overlays/category.json"),
            include_str!("../../../fixtures/overlays/tracking-https.json"),
            include_str!("../../../fixtures/overlays/pricing.json"),
            include_str!("../../../fixtures/overlays/extension-https.json"),
            include_str!("../../../fixtures/overlays/interactive-creative-file-https.json"),
            include_str!("../../../fixtures/overlays/universal-ad-id.json"),
            include_str!("../../../fixtures/overlays/api-framework.json"),
            include_str!("../../../fixtures/overlays/creative-adid.json"),
            include_str!("../../../fixtures/overlays/interactive-creative-file-type.json"),
            include_str!("../../../fixtures/overlays/viewable-impression-id.json"),
            include_str!("../../../fixtures/overlays/companion-type.json"),
            include_str!("../../../fixtures/overlays/icon-type.json"),
            include_str!("../../../fixtures/overlays/ad-id.json"),
            include_str!("../../../fixtures/overlays/mediafile-id.json"),
            include_str!("../../../fixtures/overlays/companion-adslot-id.json"),
            include_str!("../../../fixtures/overlays/icon-alt-text.json"),
            include_str!("../../../fixtures/overlays/companion-id.json"),
            include_str!("../../../fixtures/overlays/icon-hover-text.json"),
            include_str!("../../../fixtures/overlays/nonlinear-id.json"),
            include_str!("../../../fixtures/overlays/companion-rendering-mode.json"),
            include_str!("../../../fixtures/overlays/icon-pxratio.json"),
            include_str!("../../../fixtures/overlays/nonlinear-scalable.json"),
            include_str!("../../../fixtures/overlays/companion-expanded-width.json"),
            include_str!("../../../fixtures/overlays/companion-expanded-height.json"),
            include_str!("../../../fixtures/overlays/nonlinear-maintain-aspect-ratio.json"),
            include_str!("../../../fixtures/overlays/companion-asset-width.json"),
            include_str!("../../../fixtures/overlays/companion-asset-height.json"),
            include_str!("../../../fixtures/overlays/nonlinear-expanded-width.json"),
            include_str!("../../../fixtures/overlays/nonlinear-expanded-height.json"),
            include_str!("../../../fixtures/overlays/companion-xml-encoded.json"),
            include_str!("../../../fixtures/overlays/companion-pxratio.json"),
            include_str!("../../../fixtures/overlays/nonlinear-width.json"),
            include_str!("../../../fixtures/overlays/nonlinear-height.json"),
            include_str!("../../../fixtures/overlays/wrapper-follow-additional-wrappers.json"),
            include_str!("../../../fixtures/overlays/wrapper-allow-multiple-ads.json"),
            include_str!("../../../fixtures/overlays/wrapper-fallback-on-no-ad.json"),
            include_str!("../../../fixtures/overlays/ad-conditional-ad.json"),
            include_str!("../../../fixtures/overlays/mediafile-scalable.json"),
            include_str!("../../../fixtures/overlays/mediafile-maintain-aspect-ratio.json"),
            include_str!("../../../fixtures/overlays/executable-resource-type.json"),
            include_str!("../../../fixtures/overlays/javascript-resource-browser-optional.json"),
            include_str!("../../../fixtures/overlays/extension-type.json"),
            include_str!("../../../fixtures/overlays/mezzanine-type.json"),
            include_str!("../../../fixtures/overlays/mezzanine-delivery.json"),
            include_str!("../../../fixtures/overlays/ad-parameters-xml-encoded.json"),
            include_str!("../../../fixtures/overlays/mezzanine-codec.json"),
            include_str!("../../../fixtures/overlays/mezzanine-file-size.json"),
            include_str!("../../../fixtures/overlays/mezzanine-width.json"),
            include_str!("../../../fixtures/overlays/mezzanine-height.json"),
            include_str!("../../../fixtures/overlays/mezzanine-min-bitrate.json"),
            include_str!("../../../fixtures/overlays/mezzanine-max-bitrate.json"),
            include_str!("../../../fixtures/overlays/mezzanine-media-type.json"),
            include_str!("../../../fixtures/overlays/mezzanine-scalable.json"),
            include_str!("../../../fixtures/overlays/mezzanine-maintain-aspect-ratio.json"),
            include_str!("../../../fixtures/overlays/mezzanine-id.json"),
            include_str!("../../../fixtures/overlays/icon-xml-encoded.json"),
            include_str!("../../../fixtures/overlays/nonlinear-xml-encoded.json"),
            include_str!("../../../fixtures/overlays/icon-width.json"),
            include_str!("../../../fixtures/overlays/icon-height.json"),
            include_str!("../../../fixtures/overlays/caption-file-status.json"),
            include_str!("../../../fixtures/overlays/icon-click-fallback-width.json"),
            include_str!("../../../fixtures/overlays/icon-click-fallback-height.json"),
            include_str!("../../../fixtures/overlays/advertiser-id.json"),
            include_str!("../../../fixtures/overlays/companion-alt-text.json"),
            include_str!("../../../fixtures/overlays/verification-parameters-https.json"),
            include_str!("../../../fixtures/overlays/ad-type.json"),
            include_str!("../../../fixtures/overlays/impression-id.json"),
            include_str!("../../../fixtures/overlays/companion-ads-required.json"),
            include_str!("../../../fixtures/overlays/mediafile-file-size.json"),
            include_str!("../../../fixtures/overlays/mediafile-media-type.json"),
            include_str!(
                "../../../fixtures/overlays/interactive-creative-file-variable-duration.json"
            ),
            include_str!("../../../fixtures/overlays/creative-id.json"),
            include_str!("../../../fixtures/overlays/clickthrough-id.json"),
            include_str!("../../../fixtures/overlays/tracking-offset.json"),
            include_str!("../../../fixtures/overlays/clicktracking-id.json"),
            include_str!("../../../fixtures/overlays/custom-click-id.json"),
            include_str!("../../../fixtures/overlays/icon-clickthrough-id.json"),
            include_str!("../../../fixtures/overlays/companion-clickthrough-id.json"),
            include_str!("../../../fixtures/overlays/nonlinear-clickthrough-id.json"),
            include_str!("../../../fixtures/overlays/icon-clicktracking-id.json"),
            include_str!("../../../fixtures/overlays/companion-clicktracking-id.json"),
            include_str!("../../../fixtures/overlays/nonlinear-clicktracking-id.json"),
            include_str!("../../../fixtures/overlays/icon-viewtracking-id.json"),
            include_str!("../../../fixtures/overlays/tracking-id.json"),
            include_str!("../../../fixtures/overlays/error-id.json"),
            include_str!("../../../fixtures/overlays/viewable-id.json"),
            include_str!("../../../fixtures/overlays/notviewable-id.json"),
            include_str!("../../../fixtures/overlays/viewundetermined-id.json"),
            include_str!("../../../fixtures/overlays/survey-id.json"),
            include_str!("../../../fixtures/overlays/skipoffset.json"),
            include_str!("../../../fixtures/overlays/duration.json"),
            include_str!("../../../fixtures/overlays/expires.json"),
            include_str!("../../../fixtures/overlays/icon-duration.json"),
            include_str!("../../../fixtures/overlays/icon-offset.json"),
            include_str!("../../../fixtures/overlays/min-suggested-duration.json"),
            include_str!("../../../fixtures/overlays/ad-serving-id.json"),
            include_str!("../../../fixtures/overlays/ad-title.json"),
            include_str!("../../../fixtures/overlays/description.json"),
            include_str!("../../../fixtures/overlays/ad-parameters.json"),
            include_str!("../../../fixtures/overlays/static-resource-creative-type.json"),
            include_str!("../../../fixtures/overlays/flash-resource-creative-type.json"),
            include_str!("../../../fixtures/overlays/impression.json"),
            include_str!("../../../fixtures/overlays/error.json"),
            include_str!("../../../fixtures/overlays/clickthrough.json"),
            include_str!("../../../fixtures/overlays/survey.json"),
            include_str!("../../../fixtures/overlays/tracking.json"),
            include_str!("../../../fixtures/overlays/viewable.json"),
            include_str!("../../../fixtures/overlays/clicktracking.json"),
            include_str!("../../../fixtures/overlays/mezzanine.json"),
            include_str!("../../../fixtures/overlays/mediafile.json"),
            include_str!("../../../fixtures/overlays/vast-ad-tag-uri.json"),
            include_str!("../../../fixtures/overlays/javascript-resource.json"),
            include_str!("../../../fixtures/overlays/closed-caption-file.json"),
            include_str!("../../../fixtures/overlays/executable-resource.json"),
            include_str!("../../../fixtures/overlays/icon-clickthrough.json"),
            include_str!("../../../fixtures/overlays/companion-clickthrough.json"),
            include_str!("../../../fixtures/overlays/nonlinear-clickthrough.json"),
            include_str!("../../../fixtures/overlays/icon-viewtracking.json"),
            include_str!("../../../fixtures/overlays/interactive-creative-file.json"),
            include_str!("../../../fixtures/overlays/static-resource.json"),
            include_str!("../../../fixtures/overlays/iframe-resource.json"),
            include_str!("../../../fixtures/overlays/html-resource.json"),
            include_str!("../../../fixtures/overlays/flash-resource.json"),
            include_str!("../../../fixtures/overlays/icon-clickfallback.json"),
            include_str!("../../../fixtures/overlays/custom-click.json"),
            include_str!("../../../fixtures/overlays/icon-clicktracking.json"),
            include_str!("../../../fixtures/overlays/companion-clicktracking.json"),
            include_str!("../../../fixtures/overlays/nonlinear-clicktracking.json"),
            include_str!("../../../fixtures/overlays/notviewable.json"),
            include_str!("../../../fixtures/overlays/viewundetermined.json"),
            include_str!("../../../fixtures/overlays/extension.json"),
            include_str!("../../../fixtures/overlays/verification-parameters.json"),
            include_str!("../../../fixtures/overlays/blocked-ad-categories.json"),
            include_str!("../../../fixtures/overlays/creative-extension.json"),
            include_str!("../../../fixtures/overlays/creative-extension-type.json"),
            include_str!("../../../fixtures/overlays/id-value.json"),
            include_str!("../../../fixtures/overlays/mediafile-codec.json"),
            include_str!("../../../fixtures/overlays/max-width.json"),
            include_str!("../../../fixtures/overlays/max-height.json"),
            include_str!("../../../fixtures/overlays/creative-extension-https.json"),
            include_str!("../../../fixtures/overlays/notviewable-https.json"),
            include_str!("../../../fixtures/overlays/viewundetermined-https.json"),
            include_str!("../../../fixtures/overlays/custom-click-https.json"),
            include_str!("../../../fixtures/overlays/icon-clicktracking-https.json"),
            include_str!("../../../fixtures/overlays/companion-clicktracking-https.json"),
            include_str!("../../../fixtures/overlays/nonlinear-clicktracking-https.json"),
            include_str!("../../../fixtures/overlays/blocked-ad-categories-authority.json"),
            include_str!("../../../fixtures/overlays/mediafile-minbitrate.json"),
            include_str!("../../../fixtures/overlays/mediafile-maxbitrate.json"),
            include_str!("../../../fixtures/overlays/mezzanine-minbitrate-attr.json"),
            include_str!("../../../fixtures/overlays/mezzanine-maxbitrate-attr.json"),
            include_str!("../../../fixtures/overlays/mediafile-bitrate.json"),
            include_str!("../../../fixtures/overlays/mezzanine-bitrate-attr.json"),
            include_str!("../../../fixtures/overlays/mediafile-filesize.json"),
            include_str!("../../../fixtures/overlays/mezzanine-filesize.json"),
            include_str!("../../../fixtures/overlays/mediafile-width.json"),
            include_str!("../../../fixtures/overlays/mediafile-height.json"),
            include_str!("../../../fixtures/overlays/mezzanine-width-attr.json"),
            include_str!("../../../fixtures/overlays/mezzanine-height-attr.json"),
            include_str!("../../../fixtures/overlays/mediafile-delivery.json"),
            include_str!("../../../fixtures/overlays/icon-width-attr.json"),
            include_str!("../../../fixtures/overlays/icon-height-attr.json"),
            include_str!("../../../fixtures/overlays/companion-width-attr.json"),
            include_str!("../../../fixtures/overlays/companion-height-attr.json"),
            include_str!("../../../fixtures/overlays/nonlinear-width-attr.json"),
            include_str!("../../../fixtures/overlays/nonlinear-height-attr.json"),
            include_str!("../../../fixtures/overlays/companion-expanded-width-attr.json"),
            include_str!("../../../fixtures/overlays/companion-expanded-height-attr.json"),
            include_str!("../../../fixtures/overlays/nonlinear-expanded-width-attr.json"),
            include_str!("../../../fixtures/overlays/nonlinear-expanded-height-attr.json"),
            include_str!("../../../fixtures/overlays/companion-asset-width-attr.json"),
            include_str!("../../../fixtures/overlays/companion-asset-height-attr.json"),
            include_str!("../../../fixtures/overlays/icon-click-fallback-width-attr.json"),
            include_str!("../../../fixtures/overlays/icon-click-fallback-height-attr.json"),
            include_str!("../../../fixtures/overlays/mediafile-type.json"),
            include_str!("../../../fixtures/overlays/mezzanine-max-width.json"),
            include_str!("../../../fixtures/overlays/mezzanine-max-height.json"),
            include_str!("../../../fixtures/overlays/companion-max-width.json"),
            include_str!("../../../fixtures/overlays/companion-max-height.json"),
            include_str!("../../../fixtures/overlays/icon-max-width.json"),
            include_str!("../../../fixtures/overlays/icon-max-height.json"),
            include_str!("../../../fixtures/overlays/nonlinear-max-width.json"),
            include_str!("../../../fixtures/overlays/nonlinear-max-height.json"),
            include_str!("../../../fixtures/overlays/icon-click-fallback-max-width.json"),
            include_str!("../../../fixtures/overlays/icon-click-fallback-max-height.json"),
            include_str!("../../../fixtures/overlays/companion-expanded-max-width.json"),
            include_str!("../../../fixtures/overlays/companion-expanded-max-height.json"),
            include_str!("../../../fixtures/overlays/companion-asset-max-width.json"),
            include_str!("../../../fixtures/overlays/companion-asset-max-height.json"),
            include_str!("../../../fixtures/overlays/nonlinear-expanded-max-width.json"),
            include_str!("../../../fixtures/overlays/nonlinear-expanded-max-height.json"),
        ] {
            assert_schema(
                "policy-overlay.json",
                &serde_json::from_str(overlay).unwrap(),
            );
        }
        assert_schema(
            "policy-document.json",
            &serde_json::from_str(include_str!("../../../fixtures/expired-pack.json")).unwrap(),
        );
        let catalog: Value =
            serde_json::from_str(include_str!("../../../fixtures/overlays/catalog.json")).unwrap();
        for env in catalog.as_array().unwrap() {
            assert_schema("policy-overlay.json", env);
        }
    }

    #[test]
    fn claims_prepared_opportunity_observe_openrtb() {
        assert_schema(
            "operational-claims.json",
            &serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap(),
        );
        assert_schema(
            "operational-claims.json",
            &serde_json::from_str(include_str!("../../../fixtures/claims-incomplete.json"))
                .unwrap(),
        );
        assert_schema(
            "prepared-media.json",
            &serde_json::from_str(include_str!("../../../fixtures/prepared-ok.json")).unwrap(),
        );
        assert_schema(
            "opportunity.json",
            &serde_json::from_str(include_str!("../../../fixtures/eligibility/break-15s.json"))
                .unwrap(),
        );
        assert_schema(
            "opportunity.json",
            &serde_json::from_str(include_str!("../../../fixtures/eligibility/break-30s.json"))
                .unwrap(),
        );
        for obs in [
            include_str!("../../../fixtures/observe/pattern-a.json"),
            include_str!("../../../fixtures/observe/pattern-b.json"),
            include_str!("../../../fixtures/observe/pattern-c.json"),
            include_str!("../../../fixtures/observe/pattern-d.json"),
            include_str!("../../../fixtures/observe/pattern-e.json"),
        ] {
            assert_schema("observation.json", &serde_json::from_str(obs).unwrap());
        }
        assert_schema(
            "openrtb-lite.json",
            &serde_json::from_str(include_str!("../../../fixtures/observe/openrtb-lite.json"))
                .unwrap(),
        );
        let paths: Value = serde_json::from_str(include_str!(
            "../../../fixtures/environments/three-path.json"
        ))
        .unwrap();
        for env in paths.as_array().unwrap() {
            assert_schema("environment-spec.json", env);
        }
        let authorities: Value = serde_json::from_str(include_str!(
            "../../../fixtures/authorities/marketplace-publisher.json"
        ))
        .unwrap();
        for spec in authorities.as_array().unwrap() {
            assert_schema("authority-spec.json", spec);
        }
    }

    #[test]
    fn emitted_objects_match_schemas() {
        let xml = include_str!("../../../fixtures/ok-mezzanine.xml");
        let claims: OperationalClaims =
            serde_json::from_str(include_str!("../../../fixtures/claims-ok.json")).unwrap();
        let ev = evaluate(EvaluateRequest {
            vast_xml: xml,
            pack: Pack::LiveSsai,
            claims: Some(claims.clone()),
            transaction: Some("programmatic".into()),
            ..Default::default()
        });
        let evj = serde_json::to_value(&ev).unwrap();
        assert_schema("readiness-evidence.json", &evj);
        assert_schema("creative-identity.json", &evj["identity"]);
        assert_schema("media-facts.json", &evj["facts"]);
        assert_schema("validation-authority.json", &evj["authority"]);
        assert_schema("operational-claims.json", &evj["operational_claims"]);

        let g = gate(&ev, None, ev.issued_at);
        assert_schema("bid-gate.json", &serde_json::to_value(&g).unwrap());
        assert_schema(
            "readiness-summary.json",
            &serde_json::to_value(&summarize(&ev, None)).unwrap(),
        );
        assert_schema(
            "capability-surface.json",
            &serde_json::to_value(&module_surface()).unwrap(),
        );

        let media: PreparedMedia =
            serde_json::from_str(include_str!("../../../fixtures/prepared-ok.json")).unwrap();
        assert_schema(
            "prepared-media.json",
            &serde_json::to_value(&media).unwrap(),
        );

        let d = diff_vast(
            include_str!("../../../fixtures/ok-https-mp4.xml"),
            include_str!("../../../fixtures/ok-https-mp4-tracking.xml"),
        );
        assert_schema("diff-result.json", &serde_json::to_value(&d).unwrap());

        let opp: Opportunity =
            serde_json::from_str(include_str!("../../../fixtures/eligibility/break-15s.json"))
                .unwrap();
        let el = crate::eligibility(&ev, &opp, ev.issued_at);
        assert_schema("eligibility.json", &serde_json::to_value(&el).unwrap());

        let obs = observe(Observation {
            vast_xml: Some(include_str!("../../../fixtures/ok-https-mp4.xml").into()),
            identity: None,
            pack: Pack::None,
            treatment: SellerTreatment::Defer,
            pattern: Some("A".into()),
            won: Some(false),
            authority_id: None,
            auction_context: None,
            openrtb_json: Some(include_str!("../../../fixtures/observe/openrtb-lite.json").into()),
            overlays: Vec::new(),
        });
        let obsj = serde_json::to_value(&obs).unwrap();
        assert_schema("readiness-evidence.json", &obsj);
        assert_schema("auction-context.json", &obsj["auction_context"]);

        let deal =
            crate::parse_overlay(include_str!("../../../fixtures/overlays/deal-premium.json"))
                .unwrap();
        let paths = evaluate_paths(
            xml,
            Some(claims),
            &[EnvironmentSpec {
                id: "ssai-a".into(),
                pack: Pack::LiveSsai,
                overlays: vec![deal],
            }],
        );
        let pj = serde_json::to_value(&paths).unwrap();
        assert_schema("readiness-evidence.json", &pj);
        assert_schema("environment-result.json", &pj["environments"][0]);

        let _ = prebid_gate(&ev, Some(&opp));
    }

    #[test]
    fn unknown_requirement_kind_is_rejected() {
        let bad = serde_json::json!({
            "id": "x",
            "version": "1",
            "requirements": [{
                "id": "x.mystery",
                "kind": "not-a-real-kind",
                "message": "nope"
            }]
        });
        let schema = load_file("policy-document.json");
        let validator = jsonschema::options()
            .with_retriever(CatalogRetriever)
            .build(&schema)
            .unwrap();
        assert!(validator.validate(&bad).is_err());
    }

    #[test]
    fn evaluate_request_schema_accepts_context_and_custom_policy() {
        let policy: Value =
            serde_json::from_str(include_str!("../../../fixtures/expired-pack.json")).unwrap();
        let instance = serde_json::json!({
            "xml": "<VAST></VAST>",
            "pack": "none",
            "deal_id": "deal-9",
            "event_id": "game-1",
            "device_family": "ctv",
            "entry_path": "auction_triggered",
            "policy": policy
        });
        assert_schema("evaluate-request.json", &instance);
        let macros_missing = serde_json::json!({
            "id": "x",
            "version": "1",
            "requirements": [{
                "id": "x.macros",
                "kind": "require_macros",
                "message": "need values"
            }]
        });
        let schema = load_file("policy-document.json");
        let validator = jsonschema::options()
            .with_retriever(CatalogRetriever)
            .build(&schema)
            .unwrap();
        assert!(validator.validate(&macros_missing).is_err());
    }
}
