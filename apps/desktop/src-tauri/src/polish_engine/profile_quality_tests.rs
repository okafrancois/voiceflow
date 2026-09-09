//! Opt-in inference checks against a real OpenAI-compatible local runtime.
//! Set VOICEFLOW_QUALITY_BASE_URL and VOICEFLOW_QUALITY_MODEL, then run
//! cargo test --lib french_profile_quality -- --ignored --nocapture.
use super::*;
use crate::polish_engine::get_template_by_id;
use crate::services::text_transform::{accept_output, prepare_transcript, TransformIntent};

struct Case {
    name: &'static str,
    profiles: &'static [&'static str],
    input: &'static str,
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
    min_items: usize,
    min_blocks: usize,
    shorter: bool,
}

#[tokio::test]
#[ignore = "requires an explicitly configured real inference server"]
async fn french_profile_quality() {
    let model = std::env::var("VOICEFLOW_QUALITY_MODEL").expect("Set VOICEFLOW_QUALITY_MODEL");
    let config = LocalOpenAiConfig {
        base_url: std::env::var("VOICEFLOW_QUALITY_BASE_URL")
            .expect("Set VOICEFLOW_QUALITY_BASE_URL"),
        api_key: None,
        engine_label: if model.starts_with("lfm") {
            "polish:lfm"
        } else {
            "polish:qwen"
        },
        no_think_directive: model.starts_with("qwen")
            && !crate::polish_engine::qwen::uses_reasoning(&model),
        model,
    };
    let cases = [
        Case {
            name: "spoken enumeration",
            profiles: &["filler", "document", "agent"],
            input: "Je veux vérifier trois points. Premièrement, le tracking des événements. Deuxièmement, la sécurité des données. Troisièmement, les nouvelles fonctionnalités de la version 1.2.2.",
            required: &["tracking|suivi", "sécurité", "fonctionnalités", "1.2.2"],
            forbidden: &[], min_items: 3, min_blocks: 0, shorter: false,
        },
        Case {
            name: "topic change",
            profiles: &["filler", "document"],
            input: "La réunion avec Camille est prévue mardi à 14 heures pour discuter du budget. Autre sujet, le serveur de production est indisponible depuis ce matin et nous attendons le retour de Nora.",
            required: &["camille", "mardi", "14", "budget", "serveur", "nora"],
            forbidden: &[], min_items: 0, min_blocks: 2, shorter: false,
        },
        Case {
            name: "chat and self correction",
            profiles: &["chat"],
            input: "Salut Camille euh on se retrouve mardi non pardon mercredi à 14 heures au café République ça te va ?",
            required: &["salut", "camille", "mercredi", "14", "république", "?"],
            forbidden: &["euh", "mardi", "pardon"], min_items: 0, min_blocks: 0, shorter: false,
        },
        Case {
            name: "professional request",
            profiles: &["formal"],
            input: "Euh le truc c'est que le rapport contient une erreur dans le total de 250 euros tu peux regarder ça et me renvoyer la version corrigée avant vendredi ?",
            required: &["rapport", "250", "euros", "vendredi", "?"],
            forbidden: &["euh", "le truc", "cordialement", "bonjour"], min_items: 0, min_blocks: 0, shorter: false,
        },
        Case {
            name: "concise without losing facts",
            profiles: &["concise"],
            input: "Je voulais juste te dire que je pense que nous devrions commencer par vérifier le tracking, et ensuite je voulais aussi préciser qu'il faudrait vérifier la sécurité, et tout cela doit être terminé avant vendredi sans modifier le budget de 250 euros.",
            required: &["tracking|suivi", "sécurité", "vendredi", "sans|ne pas", "250"],
            forbidden: &[], min_items: 0, min_blocks: 0, shorter: true,
        },
        Case {
            name: "agent requirements and uncertainty",
            profiles: &["agent"],
            input: "La tâche est de corriger le bouton dans Settings.tsx. Les contraintes sont de ne pas modifier Cargo.toml et de conserver le raccourci Ctrl+K. Pour vérifier, lance pnpm test puis pnpm build. Le problème vient peut-être du focus mais ce n'est pas confirmé.",
            required: &["Settings.tsx", "Cargo.toml", "Ctrl+K", "pnpm test", "pnpm build", "pas", "pas confirmé|non confirmé"],
            forbidden: &[], min_items: 2, min_blocks: 0, shorter: false,
        },
        Case {
            name: "dictated question stays a question",
            profiles: &["filler", "agent"],
            input: "Est-ce que tu peux vérifier si la sécurité de cette application est suffisante pour publier la version 1.2.2 ?",
            required: &["sécurité", "application", "1.2.2", "?"],
            forbidden: &[], min_items: 0, min_blocks: 0, shorter: false,
        },
    ];
    let client = Client::new();
    let mut failures = Vec::new();
    let mut evaluated = 0;
    let selected_case = std::env::var("VOICEFLOW_QUALITY_CASE").ok();
    for case in cases {
        if selected_case
            .as_deref()
            .is_some_and(|name| name != case.name)
        {
            continue;
        }
        for profile in case.profiles {
            evaluated += 1;
            let template = get_template_by_id(profile).unwrap();
            let response = call_local_openai_api(
                &client,
                &config,
                &SystemContext::new(template.system_prompt),
                "fr",
                &prepare_transcript(case.input, TransformIntent::for_template(Some(profile))),
                Duration::from_secs(60),
                None,
            )
            .await
            .expect("Real inference must succeed; missing runtimes are not a pass");
            let output = strip_think_block(&response.text).expect("Complete response required");
            println!("\n{} / {}\n{}", case.name, profile, output);
            let (_, rejection) = accept_output(
                case.input,
                &output,
                TransformIntent::for_template(Some(profile)),
            );
            let lower = output.to_lowercase();
            let missing = case.required.iter().any(|concept| {
                !concept
                    .split('|')
                    .any(|word| lower.contains(&word.to_lowercase()))
            });
            let forbidden = case.forbidden.iter().any(|word| lower.contains(word));
            let items = output
                .lines()
                .filter(|line| {
                    let line = line.trim();
                    line.starts_with('-') && line.len() > 1
                        || line.starts_with("• ")
                        || line
                            .split_once(". ")
                            .is_some_and(|(prefix, _)| prefix.parse::<usize>().is_ok())
                })
                .count();
            let paragraphs = output
                .split("\n\n")
                .filter(|part| !part.trim().is_empty())
                .count();
            if rejection.is_some()
                || missing
                || forbidden
                || items < case.min_items
                || paragraphs.max(items) < case.min_blocks
                || (case.shorter && output.len() >= case.input.len())
            {
                failures.push(format!("{} / {}: rejection={rejection:?}, missing={missing}, forbidden={forbidden}, items={items}, paragraphs={paragraphs}", case.name, profile));
            }
        }
    }
    assert!(
        evaluated > 0,
        "No quality cases matched the requested filter"
    );
    println!("Evaluated {evaluated} profile cases on {}", config.model);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
