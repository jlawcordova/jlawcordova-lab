//! palette.export and palette.import: palette files (`.pixelpal`) and adding
//! their colors to a document (docs/intents/2026-10-object-libraries/spec.md,
//! R19–R23, D4, D6). The file, the plan and the apply are in `library.rs`.

use serde_json::{json, Value};

use super::common::{flag, opt_str, str_of};
use super::library::{expand, with_ext};
use crate::context::{Ctx, DocKind};
use crate::errors::{CommandError, Result};
use crate::library::{
    apply_palette, over_cap, palette_file_text, plan_palette, read_palette_file, stem, NewColor, PaletteDifference, PalettePlan,
    PALETTE_EXT,
};
use crate::registry::{Command, Params};
use crate::session::Edit;

fn usage(message: impl Into<String>) -> crate::errors::Error {
    CommandError::usage(message).into()
}

fn summary_format(r: &Value, _: &Params) -> String {
    str_of(r.get("summary"))
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn names_of(params: &Params, key: &str) -> Vec<String> {
    params.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect()
}

// ---------------------------------------------------------------- export

fn export_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let asked = opt_str(&params, "to").unwrap_or("");
    if asked.is_empty() {
        return Err(usage("palette.export: give the palette file's path (to)"));
    }
    let to = with_ext(expand(asked), PALETTE_EXT);
    if to.exists() && !flag(&params, "replace") {
        return Err(CommandError::new(format!("{} already exists; pick another path, or replace it", to.display()), "exists").into());
    }
    if to.is_dir() {
        return Err(usage(format!("{} is a folder; a palette file is one file", to.display())));
    }
    let palette = ctx.doc(DocKind::Palette, "")?;
    crate::state::write_atomic(&to, &palette_file_text(&palette))
        .map_err(|e| CommandError::new(format!("can't write {}: {}", to.display(), e.message()), "io"))?;
    let count = |tier: &str| palette.get(tier).and_then(Value::as_object).map_or(0, |m| m.len());
    let (world, outfit) = (count("world"), count("outfit"));
    let to_text = to.to_string_lossy().into_owned();
    Ok(json!({
        "to": to_text,
        "name": stem(&to, PALETTE_EXT),
        "world": world,
        "outfit": outfit,
        "summary": format!("Exported {} ({world} world, {outfit} outfit) to {to_text}", plural(world + outfit, "color")),
    }))
}

// ---------------------------------------------------------------- import

fn tier_cap(tier: &str) -> usize {
    if tier == "outfit" {
        engine::engine::CAPS.outfit
    } else {
        engine::engine::CAPS.world
    }
}

/// The plan as the result and the app's sheet read it (spec D6): `selected`
/// says which of the new colors `only` keeps.
fn plan_json(
    doc: &engine::engine::Sources,
    plan: &PalettePlan,
    chosen: &[NewColor],
    notes: &std::collections::BTreeMap<String, String>,
    file: &std::path::Path,
) -> Value {
    let count = |tier: &str| doc.colors.iter().filter(|(_, c)| c.tier == tier).count();
    let adding = |tier: &str| chosen.iter().filter(|c| c.tier == tier).count();
    let (over_world, over_outfit) = over_cap(doc, chosen);
    let new: Vec<Value> = plan
        .new
        .iter()
        .map(|c| {
            let mut row = json!({ "name": c.name, "tier": c.tier, "hex": c.hex, "selected": chosen.contains(c) });
            if let Some(note) = notes.get(&c.name) {
                row["note"] = json!(note);
            }
            row
        })
        .collect();
    let differing: Vec<Value> =
        plan.differing.iter().map(|d| json!({ "name": d.name, "tier": d.tier, "theirs": d.theirs, "mine": d.mine })).collect();
    let other_tier: Vec<Value> = plan
        .other_tier
        .iter()
        .map(|d| json!({ "name": d.name, "tier": d.tier, "theirsTier": d.theirs_tier, "theirs": d.theirs, "mine": d.mine }))
        .collect();
    let tier = |t: &str, over: usize| json!({ "have": count(t), "adding": adding(t), "cap": tier_cap(t), "over": over });
    json!({
        "file": { "path": file.to_string_lossy(), "name": stem(file, PALETTE_EXT) },
        "new": new,
        "differing": differing,
        "otherTier": other_tier,
        "tiers": { "world": tier("world", over_world), "outfit": tier("outfit", over_outfit) },
        "over": over_world + over_outfit,
        "ready": plan.differing.is_empty() && over_world + over_outfit == 0,
    })
}

fn refuse(code: &str, message: String, plan: &Value) -> crate::errors::Error {
    CommandError::new(message, code).with_details(plan.clone()).into()
}

fn import_run(ctx: &mut Ctx, params: Params) -> Result<Value> {
    let asked = opt_str(&params, "from").unwrap_or("");
    if asked.is_empty() {
        return Err(usage("palette.import: give the palette file's path (from)"));
    }
    let path = expand(asked);
    let file = read_palette_file(&path)?;
    let loaded = ctx.sources()?;
    let doc = &loaded.sources;
    let plan = plan_palette(doc, &file);
    let (keep, theirs, only) = (names_of(&params, "keep"), names_of(&params, "useTheirs"), names_of(&params, "only"));
    for name in keep.iter().chain(&theirs) {
        if !plan.differing.iter().any(|d| &d.name == name) && !plan.other_tier.iter().any(|d| &d.name == name) {
            return Err(usage(format!("{name} isn't a color that differs between the file and the document")));
        }
    }
    for name in &theirs {
        if plan.other_tier.iter().any(|d| &d.name == name) {
            return Err(usage(format!("{name} is in another tier in the file; it stays as the document has it")));
        }
        if keep.contains(name) {
            return Err(usage(format!("{name} is in both keep and useTheirs")));
        }
    }
    for name in &only {
        if !plan.new.iter().any(|c| &c.name == name) {
            return Err(usage(format!("{name} isn't a new color of this file, so only can't pick it")));
        }
    }
    let chosen: Vec<NewColor> = plan.new.iter().filter(|c| only.is_empty() || only.contains(&c.name)).cloned().collect();
    let shown = plan_json(doc, &plan, &chosen, &file.color_notes, &path);
    let mut shown = shown;
    if flag(&params, "dryRun") {
        shown["dryRun"] = json!(true);
        shown["summary"] =
            json!(format!("Would import {} from {}; nothing changed", plural(chosen.len(), "new color"), stem(&path, PALETTE_EXT)));
        return Ok(shown);
    }
    let undecided: Vec<&PaletteDifference> =
        plan.differing.iter().filter(|d| !keep.contains(&d.name) && !theirs.contains(&d.name)).collect();
    if !undecided.is_empty() {
        let list: Vec<String> = undecided.iter().map(|d| format!("{} ({} here, {} in the file)", d.name, d.mine, d.theirs)).collect();
        return Err(refuse(
            "palette_differs",
            format!(
                "the file has other values for {}. Give keep or useTheirs for each; using theirs changes how every object that uses the color looks.",
                list.join(", ")
            ),
            &shown,
        ));
    }
    let over = shown["over"].as_u64().unwrap_or(0);
    if over > 0 {
        let tiers: Vec<String> = ["world", "outfit"]
            .iter()
            .filter(|t| shown["tiers"][**t]["over"].as_u64().unwrap_or(0) > 0)
            .map(|t| format!("{} over the {t} tier's cap of {}", shown["tiers"][*t]["over"], tier_cap(t)))
            .collect();
        return Err(refuse(
            "palette_full",
            format!("nothing was added: adding these colors would put {}. Narrow the import with only.", tiers.join(" and ")),
            &shown,
        ));
    }
    let chosen_theirs: Vec<PaletteDifference> = plan.differing.iter().filter(|d| theirs.contains(&d.name)).cloned().collect();
    let n = chosen.len() + chosen_theirs.len();
    if n == 0 {
        shown["changed"] = json!(false);
        shown["added"] = json!([]);
        shown["summary"] = json!(format!("Nothing to import: the document already has the colors of {}", stem(&path, PALETTE_EXT)));
        return Ok(shown);
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let label = format!("Imported {} from {name}", plural(n, "color"));
    let added: Vec<String> = chosen.iter().map(|c| c.name.clone()).collect();
    let used: Vec<String> = chosen_theirs.iter().map(|c| c.name.clone()).collect();
    let (edit_file, edit_chosen, edit_theirs, edit_label) = (file, chosen, chosen_theirs, label.clone());
    let edited = ctx.edit(DocKind::Palette, "", None, move |palette, _| {
        apply_palette(palette, &edit_file, &edit_chosen, &edit_theirs);
        Ok(Edit::new(serde_json::Map::new(), edit_label))
    })?;
    shown["changed"] = edited["changed"].clone();
    shown["doc"] = edited["doc"].clone();
    shown["added"] = json!(added);
    shown["usedTheirs"] = json!(used);
    shown["summary"] = json!(label);
    Ok(shown)
}

fn import_format(r: &Value, _: &Params) -> String {
    let mut lines = vec![str_of(r.get("summary"))];
    for c in r["new"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} {} {}{}",
            if c["selected"] == json!(false) { "skip" } else { "add " },
            str_of(c.get("tier")),
            str_of(c.get("hex")),
            format_args!(" {}", str_of(c.get("name")))
        ));
    }
    for d in r["differing"].as_array().into_iter().flatten() {
        lines.push(format!("  differs {}: {} here, {} in the file", str_of(d.get("name")), str_of(d.get("mine")), str_of(d.get("theirs"))));
    }
    for d in r["otherTier"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} is {} here and {} in the file; it stays as it is",
            str_of(d.get("name")),
            str_of(d.get("tier")),
            str_of(d.get("theirsTier"))
        ));
    }
    lines.join("\n")
}

/// This file's commands.
pub fn commands() -> Vec<Command> {
    let mut export = Command::new(
        "palette.export",
        "Export the document's palette to a new .pixelpal file",
        json!({
            "type": "object",
            "properties": {
                "to": { "type": "string", "description": "the new palette file (.pixelpal is added when missing)" },
                "replace": { "type": "boolean", "default": false, "description": "replace a file that is already there" }
            },
            "required": ["to"]
        }),
        true,
        export_run,
    );
    export.description = Some("Writes the whole palette (world and outfit tiers, names, values and color notes) as one JSON file, with a format and kind line naming it a palette. Refuses an existing file (exists) unless replace. The document doesn't change.");
    export.args = vec!["to"];
    export.writes_direct = true;
    export.examples = vec!["pixellab palette export ~/Art/coast.pixelpal"];
    export.format = Some(summary_format);

    let mut import = Command::new(
        "palette.import",
        "Add the colors of a .pixelpal file the document doesn't have",
        json!({
            "type": "object",
            "properties": {
                "from": { "type": "string", "description": "the .pixelpal file" },
                "keep": { "type": "array", "items": { "type": "string" }, "description": "colors that differ: keep the document's value" },
                "useTheirs": { "type": "array", "items": { "type": "string" }, "description": "colors that differ: take the file's value (changes how every object using it looks)" },
                "only": { "type": "array", "items": { "type": "string" }, "description": "add only these new colors" },
                "dryRun": { "type": "boolean", "default": false, "description": "return the plan and change nothing" }
            },
            "required": ["from"]
        }),
        true,
        import_run,
    );
    import.description = Some(concat!(
        "Adds each color the document doesn't have, by name, to its tier at the end of it, with the file's note for it. It never removes a color; a color with the same name and value is left alone. ",
        "A name the document has with another value is listed as differing, and the import refuses (palette_differs) until each is in keep or useTheirs; a name that is in the other tier in the file is listed in otherTier and stays as the document has it. ",
        "When adding would pass a tier's cap (32 world colors, 16 outfit colors) nothing is added: it refuses (palette_full) saying how many colors are over, and only narrows the import to a chosen list. ",
        "dryRun returns the plan. One edit, one undo step and one version (Imported 4 colors from coast.pixelpal). ",
        "An agent that hits palette_differs asks the owner with chat_ask before choosing."
    ));
    import.args = vec!["from"];
    import.examples =
        vec!["pixellab palette import ~/Art/coast.pixelpal --dry-run", "pixellab palette import ~/Art/coast.pixelpal --keep wood-3"];
    import.format = Some(import_format);

    vec![export, import]
}
