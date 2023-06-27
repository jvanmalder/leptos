use futures::{Stream, StreamExt};
use leptos::{use_context, RuntimeId, ScopeId};
use leptos_config::LeptosOptions;
use leptos_meta::MetaContext;

extern crate tracing;

#[tracing::instrument(level = "trace", fields(error), skip_all)]
fn autoreload(options: &LeptosOptions) -> String {
    let site_ip = std::env::var("LEPTOS_SITE_EXTERNAL_HOSTNAME")
        .unwrap_or(options.site_addr.ip().to_string());
    let reload_port = std::env::var("LEPTOS_SITE_EXTERNAL_PORT")
        .unwrap_or(options.reload_port.to_string());
    match std::env::var("LEPTOS_WATCH").is_ok() {
        true => format!(
            r#"
                <script crossorigin="">(function () {{
                    {}
                    var ws = new WebSocket('ws://{site_ip}:{reload_port}/live_reload');
                    ws.onmessage = (ev) => {{
                        let msg = JSON.parse(ev.data);
                        if (msg.all) window.location.reload();
                        if (msg.css) {{
                            let found = false;
                            document.querySelectorAll("link").forEach((link) => {{
                                if (link.getAttribute('href').includes(msg.css)) {{
                                    let newHref = '/' + msg.css + '?version=' + new Date().getMilliseconds();
                                    link.setAttribute('href', newHref);
                                    found = true;
                                }}
                            }});
                            if (!found) console.warn(`CSS hot-reload: Could not find a <link href=/\"${{msg.css}}\"> element`);
                        }};
                        if(msg.view) {{
                            patch(msg.view);
                        }}
                    }};
                    ws.onclose = () => console.warn('Live-reload stopped. Manual reload necessary.');
                }})()
                </script>
                "#,
            leptos_hot_reload::HOT_RELOAD_JS
        ),
        false => "".to_string(),
    }
}
#[tracing::instrument(level = "trace", fields(error), skip_all)]
pub fn html_parts(
    options: &LeptosOptions,
    meta: Option<&MetaContext>,
) -> (String, &'static str) {
    let pkg_path = &options.site_pkg_dir;
    let output_name = &options.output_name;

    // Because wasm-pack adds _bg to the end of the WASM filename, and we want to mantain compatibility with it's default options
    // we add _bg to the wasm files if cargo-leptos doesn't set the env var LEPTOS_OUTPUT_NAME at compile time
    // Otherwise we need to add _bg because wasm_pack always does.
    let mut wasm_output_name = output_name.clone();
    if std::option_env!("LEPTOS_OUTPUT_NAME").is_none() {
        wasm_output_name.push_str("_bg");
    }

    let leptos_autoreload = autoreload(options);

    let html_metadata =
        meta.and_then(|mc| mc.html.as_string()).unwrap_or_default();
    let head = format!(
        r#"<!DOCTYPE html>
            <html{html_metadata}>
                <head>
                    <meta charset="utf-8"/>
                    <meta name="viewport" content="width=device-width, initial-scale=1"/>
                    <link rel="modulepreload" href="/{pkg_path}/{output_name}.js">
                    <link rel="preload" href="/{pkg_path}/{wasm_output_name}.wasm" as="fetch" type="application/wasm" crossorigin="">
                    <script type="module">import init, {{ hydrate }} from '/{pkg_path}/{output_name}.js'; init('/{pkg_path}/{wasm_output_name}.wasm').then(hydrate);</script>
                    {leptos_autoreload}
                    "#
    );
    let tail = "</body></html>";
    (head, tail)
}

#[tracing::instrument(level = "trace", fields(error), skip_all)]
pub fn html_parts_separated(
    options: &LeptosOptions,
    meta: Option<&MetaContext>,
) -> (String, &'static str) {
    let pkg_path = &options.site_pkg_dir;
    let output_name = &options.output_name;

    // Because wasm-pack adds _bg to the end of the WASM filename, and we want to mantain compatibility with it's default options
    // we add _bg to the wasm files if cargo-leptos doesn't set the env var LEPTOS_OUTPUT_NAME
    // Otherwise we need to add _bg because wasm_pack always does. This is not the same as options.output_name, which is set regardless
    let mut wasm_output_name = output_name.clone();
    if std::env::var("LEPTOS_OUTPUT_NAME").is_err() {
        wasm_output_name.push_str("_bg");
    }

    let leptos_autoreload = autoreload(options);

    let html_metadata =
        meta.and_then(|mc| mc.html.as_string()).unwrap_or_default();
    let head = meta
        .as_ref()
        .map(|meta| meta.dehydrate())
        .unwrap_or_default();
    let head = format!(
        r#"<!DOCTYPE html>
            <html{html_metadata}>
                <head>
                    <meta charset="utf-8"/>
                    <meta name="viewport" content="width=device-width, initial-scale=1"/>
                    {head}
                    <link rel="modulepreload" href="/{pkg_path}/{output_name}.js">
                    <link rel="preload" href="/{pkg_path}/{wasm_output_name}.wasm" as="fetch" type="application/wasm" crossorigin="">
                    <script type="module">import init, {{ hydrate }} from '/{pkg_path}/{output_name}.js'; init('/{pkg_path}/{wasm_output_name}.wasm').then(hydrate);</script>
                    {leptos_autoreload}
                    "#
    );
    let tail = "</body></html>";
    (head, tail)
}

#[tracing::instrument(level = "trace", fields(error), skip_all)]
pub async fn build_async_response(
    stream: impl Stream<Item = String> + 'static,
    options: &LeptosOptions,
    runtime: RuntimeId,
    scope: ScopeId,
) -> String {
    let mut buf = String::new();
    let mut stream = Box::pin(stream);
    while let Some(chunk) = stream.next().await {
        buf.push_str(&chunk);
    }

    let cx = leptos::Scope { runtime, id: scope };
    let (head, tail) =
        html_parts_separated(options, use_context::<MetaContext>(cx).as_ref());

    // in async, we load the meta content *now*, after the suspenses have resolved
    let meta = use_context::<MetaContext>(cx);
    let head_meta = meta
        .as_ref()
        .map(|meta| meta.dehydrate())
        .unwrap_or_default();
    let body_meta = meta
        .as_ref()
        .and_then(|meta| meta.body.as_string())
        .unwrap_or_default();

    runtime.dispose();

    format!("{head}{head_meta}</head><body{body_meta}>{buf}{tail}")
}
