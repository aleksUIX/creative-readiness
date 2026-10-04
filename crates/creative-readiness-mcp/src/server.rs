use rmcp::{
    handler::server::{common::schema_for_output as output_schema, wrapper::Parameters},
    model::{Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router, Json, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::call_tool;

#[derive(Clone, Default)]
pub struct ReadinessServer;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct PolicyInput {
    #[serde(default)]
    pub policy_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct XmlPackInput {
    pub xml: String,
    #[serde(default)]
    pub pack: Option<String>,
    #[serde(default)]
    pub store_json: Option<String>,
    #[serde(default)]
    pub transaction: Option<String>,
    #[serde(default)]
    pub deal_id: Option<String>,
    #[serde(default)]
    pub publisher_id: Option<String>,
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub device_family: Option<String>,
    #[serde(default)]
    pub entry_path: Option<String>,
    #[serde(default)]
    pub auction_triggered: Option<bool>,
    #[serde(default)]
    pub policy: Option<Value>,
    #[serde(default)]
    pub policy_json: Option<String>,
    #[serde(default)]
    pub overlays: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct PrepareInput {
    pub path: String,
    #[serde(default)]
    pub pack: Option<String>,
    #[serde(default)]
    pub out_dir: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct LookupInput {
    #[serde(default)]
    pub xml: Option<String>,
    #[serde(default)]
    pub store_json: Option<String>,
    #[serde(default)]
    pub identity: Option<Value>,
    #[serde(default)]
    pub crid: Option<String>,
    #[serde(default)]
    pub fallback_crid: Option<String>,
    #[serde(default)]
    pub auction_context: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct DiffInput {
    pub old_xml: String,
    pub new_xml: String,
    #[serde(default)]
    pub pack: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct EligibilityInput {
    #[serde(default)]
    pub xml: Option<String>,
    #[serde(default)]
    pub pack: Option<String>,
    #[serde(default)]
    pub evidence: Option<Value>,
    #[serde(default)]
    pub opportunity: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct TransformInput {
    pub evidence: Value,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub prepared: Option<Value>,
    #[serde(default)]
    pub pack: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema, Default)]
pub struct EmptyInput {}

fn args(v: impl serde::Serialize) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn json_out(result: Result<Value, String>) -> Json<Value> {
    Json(result.unwrap_or_else(|e| serde_json::json!({"error": e})))
}

#[tool_router]
impl ReadinessServer {
    #[tool(
        description = "Load a builtin PolicyDocument: live-ssai, live-sgai, live-csai, or live-hybrid.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn discover_policy(&self, Parameters(input): Parameters<PolicyInput>) -> Json<Value> {
        json_out(call_tool("discover_policy", args(input)))
    }

    #[tool(
        description = "Evaluate VAST XML against a pack and return ReadinessEvidence JSON.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn evaluate_creative(&self, Parameters(input): Parameters<XmlPackInput>) -> Json<Value> {
        json_out(call_tool("evaluate_creative", args(input)))
    }

    #[tool(
        description = "Reference ffmpeg prepare() against the pack ladder. Not a transcode farm.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    async fn prepare_creative(&self, Parameters(input): Parameters<PrepareInput>) -> Json<Value> {
        json_out(call_tool("prepare_creative", args(input)))
    }

    #[tool(
        description = "Local known-creative lookup. Same media hash hits; new media misses.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn known_creative_lookup(
        &self,
        Parameters(input): Parameters<LookupInput>,
    ) -> Json<Value> {
        json_out(call_tool("known_creative_lookup", args(input)))
    }

    #[tool(
        description = "Evaluate plus optional store reuse. Attaches vastlint structural fix XML when applied. No copy rewrite.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn readiness_report(&self, Parameters(input): Parameters<XmlPackInput>) -> Json<Value> {
        json_out(call_tool("readiness_report", args(input)))
    }

    #[tool(
        description = "Diff two VAST documents into an invalidation set (tracking-only keeps transcode; media change does not).",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn diff_creative(&self, Parameters(input): Parameters<DiffInput>) -> Json<Value> {
        json_out(call_tool("diff_creative", args(input)))
    }

    #[tool(
        description = "Contextual eligibility for a break or event. Ready is not eligible-for-this-break.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn check_eligibility(
        &self,
        Parameters(input): Parameters<EligibilityInput>,
    ) -> Json<Value> {
        json_out(call_tool("check_eligibility", args(input)))
    }

    #[tool(
        description = "Option C capability surface for this module. AMA is one adapter onto it.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn capability_surface(&self, Parameters(_input): Parameters<EmptyInput>) -> Json<Value> {
        json_out(call_tool("capability_surface", serde_json::json!({})))
    }

    #[tool(
        description = "Ladder encode inherits technical readiness; media replacement mints a new identity.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = false, idempotent_hint = false, destructive_hint = false)
    )]
    async fn transform_creative(
        &self,
        Parameters(input): Parameters<TransformInput>,
    ) -> Json<Value> {
        json_out(call_tool("transform_creative", args(input)))
    }

    #[tool(
        description = "Unknown may bid; serve needs ready plus eligible. Does not choose seller treatment.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn prebid_gate(&self, Parameters(input): Parameters<EligibilityInput>) -> Json<Value> {
        json_out(call_tool("prebid_gate", args(input)))
    }

    #[tool(
        description = "Ad-ops summary: may_bid, may_serve, remediable finding ids, per-path ready/not-ready.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn summarize_readiness(
        &self,
        Parameters(input): Parameters<EligibilityInput>,
    ) -> Json<Value> {
        json_out(call_tool("summarize_readiness", args(input)))
    }

    #[tool(
        description = "Auction-triggered observe. Seller treatment is an input; the module does not choose it.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn observe_creative(&self, Parameters(input): Parameters<Value>) -> Json<Value> {
        json_out(call_tool("observe_creative", input))
    }

    #[tool(
        description = "Pick deal, publisher, event, and device overlays from a catalog. Deal first.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn select_overlays(&self, Parameters(input): Parameters<Value>) -> Json<Value> {
        json_out(call_tool("select_overlays", input))
    }

    #[tool(
        description = "Observed and pending-review rows from a local identity store. Not a hosted graph.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn review_queue(&self, Parameters(input): Parameters<LookupInput>) -> Json<Value> {
        json_out(call_tool("review_queue", args(input)))
    }

    #[tool(
        description = "Lookup seller-supplied fallback_crid. Returns prior evidence if known. Does not choose fallback.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn lookup_fallback(&self, Parameters(input): Parameters<LookupInput>) -> Json<Value> {
        json_out(call_tool("lookup_fallback", args(input)))
    }

    #[tool(
        description = "Multi-authority evaluate. Disagreement keeps both findings.",
        output_schema = output_schema::<Value>(),
        annotations(read_only_hint = true, idempotent_hint = true, destructive_hint = false)
    )]
    async fn evaluate_authorities(&self, Parameters(input): Parameters<Value>) -> Json<Value> {
        json_out(call_tool("evaluate_authorities", input))
    }
}

#[tool_handler]
impl ServerHandler for ReadinessServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "creative-readiness",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Live-event ad creative readiness tools. evaluate_creative, discover_policy, \
                 known_creative_lookup, readiness_report, diff_creative, prepare_creative, \
                 check_eligibility, capability_surface, transform_creative, prebid_gate, \
                 summarize_readiness, observe_creative, select_overlays, review_queue, \
                 lookup_fallback, evaluate_authorities. AMA is an adapter, not the standard. \
                 prepare_creative is a local ffmpeg reference probe.",
            )
    }
}

use rmcp::{transport::stdio, ServiceExt};

pub async fn run() {
    let server = ReadinessServer;
    let service = server.serve(stdio()).await.expect("MCP server failed");
    service
        .waiting()
        .await
        .expect("MCP server exited with error");
}
