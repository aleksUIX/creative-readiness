use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use creative_readiness_core::{
    apply_diff, check_eligibility, diff_vast_with_policy, discover_policy, evaluate,
    evaluate_authorities, evaluate_paths_ctx, evaluate_with_store, module_surface, observe,
    parse_overlays, parse_policy, prebid_gate, revalidate, select_overlays, summarize, transform,
    AuctionContext, AuthoritySpec, EntryPath, EnvironmentSpec, EvaluateRequest, IdentityStore,
    LookupHit, Observation, OperationalClaims, Opportunity, Pack, PathContext, PreparedMedia,
    ReadinessEvidence, TransformationKind,
};
use serde::de::DeserializeOwned;

#[derive(Parser)]
#[command(
    name = "creative-readiness",
    about = "Evaluate live-event ad creative readiness from VAST XML",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Evaluate a VAST document against a pack and print ReadinessEvidence JSON.
    Evaluate {
        /// VAST XML file (use - for stdin).
        #[arg(long, short)]
        file: PathBuf,
        /// Policy pack.
        #[arg(long, value_enum, default_value_t = PackArg::LiveSsai)]
        pack: PackArg,
        /// Validation authority id recorded on the evidence.
        #[arg(long, default_value = "local")]
        authority: String,
        /// Optional operational claims JSON (transcoded, rendition_count, ...).
        #[arg(long)]
        claims: Option<PathBuf>,
        /// Optional PreparedMedia JSON from the reference probe.
        #[arg(long)]
        prepared: Option<PathBuf>,
        /// Optional media content hash (fingerprint prefers this over URL).
        #[arg(long)]
        hash: Option<String>,
        /// Optional local identity store (JSON). Lookup then persist.
        #[arg(long)]
        store: Option<PathBuf>,
        /// Policy overlay JSON (deal/publisher). Repeatable.
        #[arg(long)]
        overlay: Vec<PathBuf>,
        /// PolicyDocument JSON. Wins over builtin pack requirements (W9).
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Multi-path EnvironmentSpec JSON array.
        #[arg(long)]
        paths: Option<PathBuf>,
        /// programmatic | direct. Recorded on evidence; not a pacing decision.
        #[arg(long)]
        transaction: Option<String>,
        #[arg(long)]
        deal: Option<String>,
        #[arg(long)]
        publisher: Option<String>,
        #[arg(long)]
        event: Option<String>,
        #[arg(long)]
        device: Option<String>,
        /// Auction-triggered entry. Deal overlays may forbid this.
        #[arg(long)]
        auction_triggered: bool,
    },
    /// Record an auction-triggered observation. Seller treatment is an input.
    Observe {
        /// Observation JSON (treatment required).
        #[arg(long, short)]
        file: PathBuf,
        /// Optional VAST file to attach before observe().
        #[arg(long)]
        vast: Option<PathBuf>,
    },
    Lookup {
        #[arg(long, short)]
        file: Option<PathBuf>,
        #[arg(long)]
        store: PathBuf,
        #[arg(long)]
        hash: Option<String>,
        #[arg(long)]
        crid: Option<String>,
        /// Seller-supplied fallback_crid. Lookup only; does not choose fallback.
        #[arg(long)]
        fallback_crid: Option<String>,
    },
    /// Apply the expiry clock and optional emergency pull to stored evidence.
    Revalidate {
        #[arg(long, short)]
        file: PathBuf,
        /// Emergency pull reason. Expires evidence immediately.
        #[arg(long)]
        pull: Option<String>,
    },
    Diff {
        #[arg(long)]
        old: PathBuf,
        #[arg(long)]
        new: PathBuf,
        #[arg(long, value_enum, default_value_t = PackArg::LiveSsai)]
        pack: PackArg,
    },
    /// Map AMA 1.1 Ad or ads[] JSON onto ReadinessEvidence.
    Ama {
        #[arg(long, short)]
        file: PathBuf,
        /// Map publisher_id / deal_id (AMA 1.2 candidate).
        #[arg(long)]
        experimental: bool,
    },
    /// Reference ffmpeg probe: transcode a mezzanine against the pack ladder.
    Prepare {
        /// Mezzanine media file.
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = PackArg::LiveSsai)]
        pack: PackArg,
        /// Directory for rendition outputs.
        #[arg(long, default_value = "target/prepared")]
        out_dir: PathBuf,
    },
    /// Contextual eligibility for a break/event. Ready is not eligible-for-this-break.
    Eligibility {
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long)]
        opportunity: PathBuf,
    },
    /// Ladder encode inherits; media replacement mints a new identity.
    Transform {
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        prepared: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = PackArg::LiveSsai)]
        pack: PackArg,
    },
    /// Apply a VAST diff onto stored evidence (W20 invalidation).
    ApplyDiff {
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long)]
        old: PathBuf,
        #[arg(long)]
        new: PathBuf,
        #[arg(long, value_enum, default_value_t = PackArg::LiveSsai)]
        pack: PackArg,
    },
    /// Option C capability surface for this module.
    Surface,
    /// DSP/SSP gate: unknown may bid; serve needs ready + eligible.
    Gate {
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long)]
        opportunity: Option<PathBuf>,
    },
    /// Ad-ops summary: dimensions, remediable ids, per-path ready/not-ready.
    Summary {
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long)]
        opportunity: Option<PathBuf>,
    },
    /// Select deal/publisher/event/device overlays from a catalog. Does not apply them.
    SelectOverlays {
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        opportunity: PathBuf,
    },
    /// Observed / pending-review rows in a local identity store.
    Queue {
        #[arg(long)]
        store: PathBuf,
    },
    /// Multi-authority evaluate. Disagreement keeps both findings.
    Authorities {
        #[arg(long, short)]
        file: PathBuf,
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        claims: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum PackArg {
    None,
    #[value(name = "live-ssai")]
    LiveSsai,
    #[value(name = "live-sgai")]
    LiveSgai,
    #[value(name = "live-csai")]
    LiveCsai,
    #[value(name = "live-hybrid")]
    LiveHybrid,
}

impl From<PackArg> for Pack {
    fn from(value: PackArg) -> Self {
        match value {
            PackArg::None => Pack::None,
            PackArg::LiveSsai => Pack::LiveSsai,
            PackArg::LiveSgai => Pack::LiveSgai,
            PackArg::LiveCsai => Pack::LiveCsai,
            PackArg::LiveHybrid => Pack::LiveHybrid,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Evaluate {
            file,
            pack,
            authority,
            claims,
            prepared,
            hash,
            store,
            overlay,
            policy,
            paths,
            transaction,
            deal,
            publisher,
            event,
            device,
            auction_triggered,
        } => {
            let xml = read_text(&file);
            let claims = claims.map(|p| read_json::<OperationalClaims>(&p));
            let prepared_media = prepared.map(|p| read_json::<PreparedMedia>(&p));
            let mut overlays = Vec::new();
            for p in &overlay {
                match parse_overlays(&read_text(p)) {
                    Ok(os) => overlays.extend(os),
                    Err(e) => {
                        eprintln!("overlay {}: {e}", p.display());
                        std::process::exit(2);
                    }
                }
            }
            let custom_policy = policy.map(|p| {
                parse_policy(&read_text(&p)).unwrap_or_else(|e| {
                    eprintln!("policy {}: {e}", p.display());
                    std::process::exit(2);
                })
            });
            let opp = Opportunity {
                deal_id: deal.clone(),
                publisher_id: publisher.clone(),
                event_id: event.clone(),
                device_family: device.clone(),
                ..Default::default()
            };
            overlays = select_overlays(&overlays, &opp);
            let evidence = if let Some(path) = paths {
                let specs: Vec<EnvironmentSpec> = read_json(&path);
                evaluate_paths_ctx(
                    &xml,
                    claims.clone(),
                    &specs,
                    &PathContext {
                        deal_id: deal.clone(),
                        publisher_id: publisher.clone(),
                        event_id: event.clone(),
                        device_family: device.clone(),
                        entry_path: if auction_triggered {
                            EntryPath::AuctionTriggered
                        } else {
                            EntryPath::Proactive
                        },
                        policy: custom_policy.clone(),
                    },
                )
            } else {
                let req = EvaluateRequest {
                    vast_xml: &xml,
                    pack: pack.into(),
                    authority_id: &authority,
                    claims,
                    prepared_media,
                    content_hash: hash,
                    overlays,
                    transaction,
                    device_family: device,
                    event_id: event,
                    deal_id: deal,
                    publisher_id: publisher,
                    entry_path: if auction_triggered {
                        EntryPath::AuctionTriggered
                    } else {
                        EntryPath::Proactive
                    },
                    policy: custom_policy,
                    ..Default::default()
                };
                if let Some(path) = store {
                    let mut store = IdentityStore::load(&path).unwrap_or_else(|e| {
                        eprintln!("store: {e}");
                        std::process::exit(2);
                    });
                    let evidence = evaluate_with_store(req, &store);
                    store.insert(evidence.identity.clone(), evidence.clone());
                    if let Err(e) = store.save(&path) {
                        eprintln!("store save: {e}");
                        return ExitCode::from(2);
                    }
                    evidence
                } else {
                    evaluate(req)
                }
            };
            print_evidence(&evidence)
        }
        Command::Observe { file, vast } => {
            let mut obs: Observation = read_json(&file);
            if let Some(path) = vast {
                obs.vast_xml = Some(read_text(&path));
            }
            print_evidence(&observe(obs))
        }
        Command::Lookup {
            file,
            store,
            hash,
            crid,
            fallback_crid,
        } => {
            let store = IdentityStore::load(&store).unwrap_or_else(|e| {
                eprintln!("store: {e}");
                std::process::exit(2);
            });
            let hit = if let Some(crid) = fallback_crid {
                store.lookup_fallback(&AuctionContext {
                    fallback_crid: Some(crid),
                    ..Default::default()
                })
            } else if let Some(crid) = crid {
                store.lookup_crid(&crid)
            } else {
                let file = file.unwrap_or_else(|| {
                    eprintln!("lookup requires --file, --crid, or --fallback-crid");
                    std::process::exit(2);
                });
                let xml = read_text(&file);
                let ev = evaluate(EvaluateRequest {
                    vast_xml: &xml,
                    pack: Pack::None,
                    content_hash: hash,
                    ..Default::default()
                });
                store.lookup(&ev.identity)
            };
            let (status, previous, evidence) = match hit {
                LookupHit::Unknown => ("unknown", None, None),
                LookupHit::Known { evidence } => ("known", None, Some(evidence)),
                LookupHit::MediaChanged {
                    previous_fingerprint,
                } => ("media_changed", Some(previous_fingerprint), None),
            };
            let out = serde_json::json!({
                "hit": status,
                "previous_fingerprint": previous,
                "evidence": evidence,
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
            if status == "known" {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Command::Diff { old, new, pack } => {
            let pack: Pack = pack.into();
            let policy = if pack.as_str() == "none" {
                None
            } else {
                discover_policy(pack.as_str()).ok()
            };
            let diff = diff_vast_with_policy(&read_text(&old), &read_text(&new), policy.as_ref());
            println!("{}", serde_json::to_string_pretty(&diff).unwrap());
            if diff.invalidations.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Command::Revalidate { file, pull } => {
            let mut ev: ReadinessEvidence = read_json(&file);
            let closed = revalidate(&mut ev, pull.as_deref());
            println!("{}", serde_json::to_string_pretty(&ev).unwrap());
            if closed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Command::Ama { file, experimental } => {
            let raw = read_text(&file);
            match creative_readiness_ama::adapt_ama(
                &raw,
                creative_readiness_ama::AdaptOptions { experimental },
            ) {
                Ok(ads) => {
                    println!("{}", serde_json::to_string_pretty(&ads).unwrap());
                    if ads
                        .iter()
                        .any(|e| e.findings.iter().any(|f| f.severity == "error"))
                    {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(2)
                }
            }
        }
        Command::Prepare {
            file,
            pack,
            out_dir,
        } => {
            let pack: Pack = pack.into();
            let policy_id = pack.as_str();
            if policy_id == "none" {
                eprintln!("prepare requires --pack live-ssai or live-sgai");
                return ExitCode::from(2);
            }
            let policy = match discover_policy(policy_id) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
            let Some(ladder) = policy.transcode_ladder else {
                eprintln!("pack {policy_id} has no transcode_ladder");
                return ExitCode::from(2);
            };
            match creative_readiness_probe::prepare(&file, &ladder, &out_dir) {
                Ok(media) => {
                    println!("{}", serde_json::to_string_pretty(&media).unwrap());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        Command::Eligibility { file, opportunity } => {
            let ev: ReadinessEvidence = read_json(&file);
            let opp: Opportunity = read_json(&opportunity);
            let el = check_eligibility(&ev, &opp);
            println!("{}", serde_json::to_string_pretty(&el).unwrap());
            if el.eligible {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Command::Transform {
            file,
            kind,
            prepared,
            pack,
        } => {
            let ev: ReadinessEvidence = read_json(&file);
            let kind = match kind.to_ascii_lowercase().as_str() {
                "encode" => TransformationKind::Encode,
                "media_replacement" | "media-replacement" => TransformationKind::MediaReplacement,
                other => {
                    eprintln!("kind {other:?}; expected encode or media_replacement");
                    return ExitCode::from(2);
                }
            };
            let prepared = prepared.map(|p| read_json::<PreparedMedia>(&p));
            let pack: Pack = pack.into();
            let policy = pack
                .as_str()
                .ne("none")
                .then(|| discover_policy(pack.as_str()).ok())
                .flatten();
            let rights = policy.as_ref().map(|p| p.transformation_rights.clone());
            let ladder = policy.as_ref().and_then(|p| p.transcode_ladder.clone());
            let out = transform(
                &ev,
                kind,
                prepared.as_ref(),
                rights.as_ref(),
                ladder.as_ref(),
            );
            print_evidence(&out)
        }
        Command::ApplyDiff {
            file,
            old,
            new,
            pack,
        } => {
            let mut ev: ReadinessEvidence = read_json(&file);
            let pack: Pack = pack.into();
            let policy = if pack.as_str() == "none" {
                None
            } else {
                discover_policy(pack.as_str()).ok()
            };
            let diff = diff_vast_with_policy(&read_text(&old), &read_text(&new), policy.as_ref());
            apply_diff(&mut ev, &diff);
            print_evidence(&ev)
        }
        Command::Surface => {
            println!(
                "{}",
                serde_json::to_string_pretty(&module_surface()).unwrap()
            );
            ExitCode::SUCCESS
        }
        Command::Gate { file, opportunity } => {
            let ev: ReadinessEvidence = read_json(&file);
            let opp = opportunity.map(|p| read_json::<Opportunity>(&p));
            let g = prebid_gate(&ev, opp.as_ref());
            println!("{}", serde_json::to_string_pretty(&g).unwrap());
            if g.may_serve {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Command::Summary { file, opportunity } => {
            let ev: ReadinessEvidence = read_json(&file);
            let opp = opportunity.map(|p| read_json::<Opportunity>(&p));
            let s = summarize(&ev, opp.as_ref());
            println!("{}", serde_json::to_string_pretty(&s).unwrap());
            if s.may_serve {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Command::SelectOverlays {
            catalog,
            opportunity,
        } => {
            let catalog: Vec<creative_readiness_core::PolicyOverlay> = read_json(&catalog);
            let opp: Opportunity = read_json(&opportunity);
            let selected = select_overlays(&catalog, &opp);
            println!("{}", serde_json::to_string_pretty(&selected).unwrap());
            ExitCode::SUCCESS
        }
        Command::Queue { store } => {
            let store = IdentityStore::load(&store).unwrap_or_else(|e| {
                eprintln!("store: {e}");
                std::process::exit(2);
            });
            let pending = store.review_queue();
            println!("{}", serde_json::to_string_pretty(&pending).unwrap());
            if pending.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Command::Authorities { file, spec, claims } => {
            let xml = read_text(&file);
            let authorities: Vec<AuthoritySpec> = read_json(&spec);
            let claims = claims.map(|p| read_json::<OperationalClaims>(&p));
            let ev = evaluate_authorities(&xml, &authorities, claims);
            print_evidence(&ev)
        }
    }
}

fn read_text(file: &PathBuf) -> String {
    if file.as_os_str() == "-" {
        std::io::read_to_string(std::io::stdin()).expect("read stdin")
    } else {
        std::fs::read_to_string(file).unwrap_or_else(|e| {
            eprintln!("read {}: {e}", file.display());
            std::process::exit(2);
        })
    }
}

fn read_json<T: DeserializeOwned>(path: &PathBuf) -> T {
    let raw = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("read {}: {e}", path.display());
        std::process::exit(2);
    });
    serde_json::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("json {}: {e}", path.display());
        std::process::exit(2);
    })
}

fn print_evidence(evidence: &creative_readiness_core::ReadinessEvidence) -> ExitCode {
    match serde_json::to_string_pretty(evidence) {
        Ok(json) => {
            println!("{json}");
            if evidence.findings.iter().any(|f| f.severity == "error") {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("serialize: {e}");
            ExitCode::from(2)
        }
    }
}
