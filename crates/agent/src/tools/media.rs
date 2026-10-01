use super::{arg, def, need, Ctx, Outcome};
use crate::part::Part;
use anyhow::Result;
use grok::ToolDef;
use serde_json::{json, Value};

pub fn defs(voice: bool) -> Vec<ToolDef> {
  let mut all = vec![
    def(
      "generate_image",
      "Generate an image from a description. Saved to the workspace and shown to the user.",
      json!({"type": "object", "properties": {"prompt": {"type": "string"}, "path": {"type": "string", "description": "optional workspace path"}}, "required": ["prompt"]}),
    ),
  ];
  if voice {
    all.push(def(
      "voice_memo",
      "Send the user a short spoken audio message (with its transcript).",
      json!({"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}),
    ));
  }
  all
}

pub async fn call(ctx: &Ctx<'_>, name: &str, args: &Value) -> Option<Result<Outcome>> {
  let ws = ctx.rt.computer.workspace();
  let r = match name {
    "generate_image" => async {
      let prompt = need(args, "prompt")?;
      let bytes = crate::media::image(ctx.rt, prompt).await?;
      let path = match arg(args, "path") {
        "" => format!("images/{}.png", store::now()),
        p => p.to_string(),
      };
      let shown = computer::fs::write_bytes(&ws, &path, &bytes)?;
      let full = computer::fs::resolve(&ws, &shown)?;
      Ok(Outcome::with(format!("Saved {shown}"), Part::Image { path: full.display().to_string(), caption: prompt.to_string() }))
    }
    .await,
    "voice_memo" if !ctx.rt.settings().voice_enabled => async { anyhow::bail!("Voice is turned off in Settings.") }.await,
    "voice_memo" => async {
      let text = need(args, "text")?;
      let voice = ctx.rt.settings().voice;
      let path = crate::media::speak(ctx.rt, &ws, text, &voice).await?;
      Ok(Outcome::with("Sent a voice memo.", Part::VoiceMemo { path: path.display().to_string(), transcript: text.to_string() }))
    }
    .await,
    _ => return None,
  };
  Some(r)
}

#[cfg(test)]
#[path = "../../tests/tools/media.rs"]
mod tests;
