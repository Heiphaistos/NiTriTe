//! Rendu des sauvegardes en TXT / Markdown / HTML
#[allow(unused_imports)]
use super::*;
use super::{BackupManifest, BackupStatus};

/// Libellé + couleur d'un statut pour l'affichage.
fn status_badge(status: &BackupStatus) -> (&'static str, &'static str) {
    match status {
        BackupStatus::Ok => ("OK", "#22c55e"),
        BackupStatus::Vide => ("VIDE", "#a1a1aa"),
        BackupStatus::Echec => ("ÉCHEC", "#ef4444"),
    }
}

pub fn render_txt(m: &BackupManifest) -> String {
    let width = 72usize;
    let border = "═".repeat(width);
    let thin   = "─".repeat(width);

    let mut out = String::new();

    // En-tête principal
    out.push_str(&format!("╔{}╗\n", border));
    let title = "N i T r i T e  —  R a p p o r t  d e  S a u v e g a r d e";
    let pad = (width.saturating_sub(title.chars().count())) / 2;
    out.push_str(&format!("║{}{}{:>pad2$}║\n",
        " ".repeat(pad), title,
        " ",
        pad2 = width.saturating_sub(pad).saturating_sub(title.chars().count())));
    out.push_str(&format!("╠{}╣\n", border));
    out.push_str(&format!("║  Date      : {:<width$}║\n", m.timestamp, width = width - 15));
    out.push_str(&format!("║  Éléments  : {:<width$}║\n", m.total_items, width = width - 15));
    out.push_str(&format!("╚{}╝\n\n", border));

    // Grouper par catégorie
    let mut current_cat = String::new();
    for item in &m.items {
        // Nouveau groupe catégorie
        if item.category != current_cat {
            current_cat = item.category.clone();
            out.push_str(&format!("┌{}┐\n", thin));
            let cat_label = format!("  {}  ", current_cat.to_uppercase());
            let cat_pad = width.saturating_sub(cat_label.chars().count());
            out.push_str(&format!("│ {:<width$}│\n", format!("▶  {}", current_cat.to_uppercase()), width = width - 1));
            out.push_str(&format!("└{}┘\n", thin));
            let _ = cat_pad;
            let _ = cat_label;
        }

        // Bloc item
        out.push_str(&format!("\n  ┌─ {} {}\n", item.name, "─".repeat(width.saturating_sub(item.name.chars().count() + 4))));

        // Données — indenter chaque ligne (convertir JSON si nécessaire)
        let readable_data = json_to_readable(&item.data);
        for line in readable_data.lines().take(200) {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                out.push_str("  │\n");
            } else {
                // Couper les lignes trop longues
                let max_line = width - 4;
                if trimmed.len() <= max_line {
                    out.push_str(&format!("  │  {}\n", trimmed));
                } else {
                    let mut remaining = trimmed;
                    let mut first = true;
                    while !remaining.is_empty() {
                        // safe char boundary
                        let cut = remaining.char_indices().nth(max_line).map(|(i, _)| i).unwrap_or(remaining.len());
                        let (chunk, rest) = remaining.split_at(cut);
                        if first {
                            out.push_str(&format!("  │  {}\n", chunk));
                            first = false;
                        } else {
                            out.push_str(&format!("  │     {}\n", chunk));
                        }
                        remaining = rest;
                    }
                }
            }
        }
        if readable_data.lines().count() > 200 {
            out.push_str(&format!("  │  … ({} lignes supplémentaires tronquées)\n",
                readable_data.lines().count() - 200));
        }
        out.push_str(&format!("  └{}\n", "─".repeat(width - 2)));
    }

    // Pied de page
    out.push('\n');
    out.push_str(&format!("╔{}╗\n", border));
    out.push_str(&format!("║  Généré par NiTriTe {:<width$}║\n",
        format!("— {} éléments exportés", m.total_items), width = width - 22));
    out.push_str(&format!("╚{}╝\n", border));

    out
}

pub fn render_md(m: &BackupManifest) -> String {
    let mut out = format!("# NiTriTe — Rapport de Sauvegarde\n\n**Date :** `{}`  \n**Total :** {} éléments\n\n---\n\n",
        m.timestamp, m.total_items);
    let mut current_cat = String::new();
    for item in &m.items {
        if item.category != current_cat {
            out.push_str(&format!("## {}\n\n", item.category));
            current_cat = item.category.clone();
        }
        out.push_str(&format!("### {}\n\n```\n{}\n```\n\n", item.name, json_to_readable(&item.data)));
    }
    out
}

pub fn render_html(m: &BackupManifest) -> String {
    // Comptage des items par catégorie
    let mut cat_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for item in &m.items {
        *cat_counts.entry(item.category.clone()).or_insert(0) += 1;
    }

    // Ordre d'apparition des catégories
    let mut cats_seen: Vec<String> = Vec::new();
    for item in &m.items {
        if !cats_seen.contains(&item.category) {
            cats_seen.push(item.category.clone());
        }
    }

    // Liens de navigation avec compteur
    let nav_links: String = cats_seen.iter().map(|c| {
        let slug = c.to_lowercase().replace(' ', "-");
        let count = cat_counts.get(c).copied().unwrap_or(0);
        format!(
            "<a href='#cat-{slug}' class='nav-link'>{c}<span class='nav-count'>{count}</span></a>",
            slug = slug, c = html_escape(c), count = count
        )
    }).collect::<Vec<_>>().join("\n");

    // Sections collapsibles (<details>/<summary> = natif, 0 JS requis)
    let mut sections = String::new();
    let mut current_cat = String::new();
    for item in &m.items {
        if item.category != current_cat {
            if !current_cat.is_empty() {
                sections.push_str("</div></details>\n");
            }
            current_cat = item.category.clone();
            let slug = current_cat.to_lowercase().replace(' ', "-");
            let count = cat_counts.get(&current_cat).copied().unwrap_or(0);
            sections.push_str(&format!(
                "<details class='section' id='cat-{slug}' open>\
<summary class='section-header'>\
<span class='section-arrow'>▶</span>\
<h2>{cat}</h2>\
<span class='section-count'>{count} élément{s}</span>\
</summary>\
<div class='cards'>\n",
                slug  = slug,
                cat   = html_escape(&current_cat),
                count = count,
                s     = if count > 1 { "s" } else { "" },
            ));
        }
        let (badge_txt, badge_col) = status_badge(&item.status);
        let body = match item.status {
            BackupStatus::Ok => format!("<pre class='card-data'>{}</pre>", html_escape(&json_to_readable(&item.data))),
            _ => format!("<pre class='card-data' style='color:var(--text2)'>{}</pre>", html_escape(&item.detail)),
        };
        let size_lbl = if item.status == BackupStatus::Ok {
            format!("<span class='card-size'>{}</span>", format_size(item.size_bytes as u64))
        } else { String::new() };
        sections.push_str(&format!(
            "<details class='card' open>\
<summary class='card-title'>\
<span class='card-arrow'>▶</span>{name}\
<span class='card-badge' style='color:{col};border-color:{col}'>{badge}</span>{size}\
</summary>\
{body}\
</details>\n",
            name = html_escape(&item.name),
            col = badge_col,
            badge = badge_txt,
            size = size_lbl,
            body = body,
        ));
    }
    if !current_cat.is_empty() {
        sections.push_str("</div></details>\n");
    }

    format!(r#"<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>NiTriTe — Rapport de Sauvegarde — {ts}</title>
<style>
  *{{box-sizing:border-box;margin:0;padding:0}}
  :root{{
    --bg:#09090b;--bg2:#111113;--bg3:#18181b;--border:#27272a;--border2:#3f3f46;
    --accent:#f97316;--text:#e4e4e7;--text2:#a1a1aa;--text3:#52525b;
    --success:#22c55e;--radius:10px;
  }}
  body{{font-family:Inter,system-ui,sans-serif;background:var(--bg);color:var(--text);
    display:flex;min-height:100vh;font-size:13px}}

  /* ── Sidebar ── */
  nav{{width:230px;flex-shrink:0;background:var(--bg2);border-right:1px solid var(--border);
    padding:0;position:sticky;top:0;height:100vh;overflow-y:auto;display:flex;flex-direction:column}}
  .nav-brand{{padding:20px;border-bottom:1px solid var(--border);flex-shrink:0}}
  .nav-brand-title{{font-size:18px;font-weight:800;
    background:linear-gradient(135deg,#fafafa 40%,#f97316);
    -webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}}
  .nav-brand-sub{{font-size:10px;color:var(--text3);font-family:'Courier New',monospace;margin-top:3px}}
  .nav-actions{{padding:10px 12px;border-bottom:1px solid var(--border);
    display:flex;gap:6px;flex-shrink:0}}
  .nav-btn{{flex:1;padding:5px 8px;border:1px solid var(--border);border-radius:6px;
    background:var(--bg3);color:var(--text2);cursor:pointer;font-size:11px;
    font-family:inherit;transition:all .15s;text-align:center}}
  .nav-btn:hover{{border-color:var(--accent);color:var(--accent)}}
  .nav-label{{padding:8px 16px 4px;font-size:9px;font-weight:700;color:var(--text3);
    text-transform:uppercase;letter-spacing:.12em}}
  .nav-link{{display:flex;align-items:center;justify-content:space-between;
    padding:7px 16px;font-size:12px;color:var(--text2);text-decoration:none;
    border-left:2px solid transparent;transition:all .15s}}
  .nav-link:hover{{color:var(--accent);border-left-color:var(--accent);
    background:rgba(249,115,22,.06)}}
  .nav-count{{font-size:10px;background:var(--bg3);border:1px solid var(--border);
    border-radius:99px;padding:1px 7px;color:var(--text3);font-weight:500}}

  /* ── Main ── */
  main{{flex:1;padding:32px 40px;min-width:0;max-width:1200px}}

  /* ── Header ── */
  .report-header{{margin-bottom:28px}}
  .report-title{{font-size:28px;font-weight:800;margin-bottom:10px;
    background:linear-gradient(135deg,#fafafa 40%,#f97316);
    -webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text}}
  .report-meta{{display:flex;gap:10px;flex-wrap:wrap}}
  .meta-pill{{display:inline-flex;align-items:center;gap:6px;padding:5px 12px;
    background:var(--bg3);border:1px solid var(--border);border-radius:99px;
    font-size:11px;color:var(--text2)}}
  .meta-pill strong{{color:var(--accent)}}

  /* ── Sections collapsibles ── */
  details.section{{margin-bottom:20px;scroll-margin-top:16px;
    border:1px solid var(--border);border-radius:var(--radius);overflow:hidden}}
  details.section > summary.section-header{{
    display:flex;align-items:center;gap:12px;
    padding:14px 18px;cursor:pointer;list-style:none;
    background:var(--bg3);border-bottom:1px solid transparent;
    user-select:none;transition:background .15s}}
  details.section > summary.section-header::-webkit-details-marker{{display:none}}
  details.section > summary:hover{{background:var(--bg2)}}
  details.section[open] > summary{{border-bottom-color:var(--border)}}
  .section-arrow{{color:var(--accent);font-size:9px;display:inline-block;
    transition:transform .2s;flex-shrink:0}}
  details.section[open] > summary .section-arrow{{transform:rotate(90deg)}}
  details.section > summary h2{{font-size:13px;font-weight:700;text-transform:uppercase;
    letter-spacing:.06em;color:var(--text2);flex:1}}
  .section-count{{font-size:10px;background:rgba(249,115,22,.12);color:var(--accent);
    border:1px solid rgba(249,115,22,.25);border-radius:99px;padding:2px 10px;
    font-weight:600;flex-shrink:0}}

  /* ── Cards collapsibles dans la section ── */
  .cards{{display:flex;flex-direction:column;gap:0;padding:12px;gap:8px}}
  details.card{{background:var(--bg2);border:1px solid var(--border);
    border-radius:8px;overflow:hidden;transition:border-color .2s}}
  details.card:hover{{border-color:var(--border2)}}
  details.card[open]{{border-color:var(--border2)}}
  details.card > summary.card-title{{
    display:flex;align-items:center;gap:10px;
    padding:10px 14px;cursor:pointer;list-style:none;
    font-size:12px;font-weight:600;color:var(--text2);
    background:var(--bg3);user-select:none;transition:color .15s;
    border-bottom:1px solid transparent}}
  details.card > summary.card-title::-webkit-details-marker{{display:none}}
  details.card > summary:hover{{color:var(--text)}}
  .card-badge{{font-size:9px;font-weight:700;letter-spacing:.05em;border:1px solid;
    border-radius:4px;padding:1px 6px;margin-left:8px;flex-shrink:0}}
  .card-size{{font-size:10px;color:var(--text3);margin-left:auto;flex-shrink:0}}
  details.card[open] > summary{{border-bottom-color:var(--border)}}
  .card-arrow{{font-size:8px;color:var(--text3);display:inline-block;
    transition:transform .2s;flex-shrink:0}}
  details.card[open] > summary .card-arrow{{transform:rotate(90deg)}}

  /* ── Contenu des cards (pre) ── */
  pre.card-data{{
    padding:14px 16px;
    font-size:11.5px;font-family:'JetBrains Mono','Courier New',monospace;
    color:var(--text);white-space:pre-wrap;word-break:break-word;
    line-height:1.7;
    /* PAS de max-height — tout le contenu est visible */
    overflow-x:auto;
    scrollbar-width:thin;scrollbar-color:var(--border2) transparent;
  }}
  pre.card-data::-webkit-scrollbar{{height:4px}}
  pre.card-data::-webkit-scrollbar-thumb{{background:var(--border2);border-radius:99px}}

  /* ── Footer ── */
  .report-footer{{margin-top:40px;padding-top:16px;border-top:1px solid var(--border);
    font-size:11px;color:var(--text3);text-align:center}}

  @media print{{
    nav{{display:none}}
    main{{padding:16px;max-width:100%}}
    details.section,details.card{{border:1px solid #ccc}}
    details.section > summary,details.card > summary{{background:#f5f5f5;color:#333}}
    pre.card-data{{color:#111;font-size:10px}}
  }}
</style>
</head>
<body>

<nav>
  <div class="nav-brand">
    <div class="nav-brand-title">NiTriTe</div>
    <div class="nav-brand-sub">Rapport de sauvegarde</div>
  </div>
  <div class="nav-actions">
    <button class="nav-btn" onclick="document.querySelectorAll('details').forEach(d=>d.open=true)">Tout ouvrir</button>
    <button class="nav-btn" onclick="document.querySelectorAll('details').forEach(d=>d.open=false)">Tout fermer</button>
  </div>
  <div class="nav-label">Catégories</div>
  {nav_links}
</nav>

<main>
  <div class="report-header">
    <div class="report-title">Rapport de Sauvegarde</div>
    <div class="report-meta">
      <div class="meta-pill">📅 Date&nbsp;<strong>{ts}</strong></div>
      <div class="meta-pill">📦 Éléments&nbsp;<strong>{total}</strong></div>
      <div class="meta-pill" style="border-color:#22c55e55">✅ Collectés&nbsp;<strong style="color:#22c55e">{ok}</strong></div>
      <div class="meta-pill">⬜ Vides&nbsp;<strong>{empty}</strong></div>
      <div class="meta-pill" style="border-color:#ef444455">⚠ Échecs&nbsp;<strong style="color:#ef4444">{failed}</strong></div>
    </div>
  </div>

  {sections}

  <div class="report-footer">
    Généré par NiTriTe &mdash; {ts} &mdash; {total} éléments exportés
  </div>
</main>

</body></html>"#,
        ts        = html_escape(&m.timestamp),
        total     = m.total_items,
        ok        = m.ok_count,
        empty     = m.empty_count,
        failed    = m.failed_count,
        nav_links = nav_links,
        sections  = sections,
    )
}

/// Rendu HTML lisible du scan PC complet (ScanResult en JSON → page structurée).
/// Les scalaires deviennent un tableau clé/valeur ; les listes/objets une section
/// dépliable avec compteur ; le JSON complet reste disponible en bas (rejouable).
pub fn render_scan_html(v: &serde_json::Value) -> String {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return "<!DOCTYPE html><html lang=\"fr\"><meta charset=\"UTF-8\"><body>Scan vide.</body></html>".to_string(),
    };

    let mut scalars = String::new();
    let mut sections = String::new();
    for (k, val) in obj {
        let label = html_escape(&prettify_key(k));
        match val {
            serde_json::Value::Array(arr) => {
                if arr.is_empty() { continue; }
                sections.push_str(&format!(
                    "<details class='sec' open><summary>{label} <span class='cnt'>{n}</span></summary><pre>{body}</pre></details>\n",
                    label = label, n = arr.len(),
                    body = html_escape(&json_to_readable(&val.to_string())),
                ));
            }
            serde_json::Value::Object(_) => {
                sections.push_str(&format!(
                    "<details class='sec' open><summary>{label}</summary><pre>{body}</pre></details>\n",
                    label = label,
                    body = html_escape(&json_to_readable(&val.to_string())),
                ));
            }
            serde_json::Value::Null => {}
            serde_json::Value::String(s) if s.is_empty() => {}
            _ => {
                let txt = match val {
                    serde_json::Value::Bool(b) => if *b { "Oui".to_string() } else { "Non".to_string() },
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                scalars.push_str(&format!(
                    "<tr><td class='k'>{}</td><td class='v'>{}</td></tr>",
                    label, html_escape(&txt)
                ));
            }
        }
    }

    format!(r#"<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>NiTriTe — Scan PC complet</title>
<style>
  *{{box-sizing:border-box;margin:0;padding:0}}
  :root{{--bg:#09090b;--bg2:#18181b;--border:#27272a;--accent:#f97316;--text:#e4e4e7;--text2:#a1a1aa}}
  body{{font-family:Inter,system-ui,sans-serif;background:var(--bg);color:var(--text);
    font-size:13px;padding:28px 32px;max-width:1100px;margin:0 auto}}
  h1{{font-size:24px;font-weight:800;margin-bottom:6px;
    background:linear-gradient(135deg,#fafafa 40%,#f97316);-webkit-background-clip:text;
    -webkit-text-fill-color:transparent;background-clip:text}}
  .sub{{color:var(--text2);font-size:12px;margin-bottom:22px}}
  table{{width:100%;border-collapse:collapse;margin-bottom:22px;
    border:1px solid var(--border);border-radius:8px;overflow:hidden}}
  td{{padding:7px 12px;border-bottom:1px solid var(--border);vertical-align:top}}
  td.k{{color:var(--text2);width:40%;font-weight:500}}
  td.v{{color:var(--text);word-break:break-word}}
  details.sec{{margin-bottom:10px;border:1px solid var(--border);border-radius:8px;overflow:hidden}}
  details.sec > summary{{padding:10px 14px;cursor:pointer;background:var(--bg2);
    font-weight:600;color:var(--text2);text-transform:uppercase;letter-spacing:.04em;font-size:12px}}
  .cnt{{font-size:10px;background:rgba(249,115,22,.12);color:var(--accent);
    border-radius:99px;padding:1px 8px;margin-left:6px}}
  pre{{padding:12px 16px;font-family:'JetBrains Mono','Courier New',monospace;font-size:11.5px;
    white-space:pre-wrap;word-break:break-word;line-height:1.7;color:var(--text)}}
</style></head><body>
  <h1>Scan PC complet</h1>
  <div class="sub">Rapport diagnostic NiTriTe — matériel, sécurité, processus, services, mises à jour</div>
  <table><tbody>{scalars}</tbody></table>
  {sections}
</body></html>"#,
        scalars = scalars,
        sections = sections,
    )
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backup::collector::{BackupManifest, BackupItem};

    fn make_manifest(items: Vec<(&str, &str, &str)>) -> BackupManifest {
        let items: Vec<BackupItem> = items.into_iter().map(|(name, cat, data)| BackupItem {
            id: name.to_string(),
            name: name.to_string(),
            category: cat.to_string(),
            size_bytes: data.len(),
            data: data.to_string(),
            status: BackupStatus::Ok,
            detail: String::new(),
            file: String::new(),
        }).collect();
        let total = items.len();
        BackupManifest {
            timestamp: "2026-01-01_12-00-00".to_string(),
            total_items: total, ok_count: total, empty_count: 0, failed_count: 0,
            items, path: "/tmp/test".to_string(),
        }
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("A&B"), "A&amp;B");
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a > b < c"), "a &gt; b &lt; c");
        assert_eq!(html_escape("normal text"), "normal text");
    }

    #[test]
    fn render_md_empty_manifest() {
        let m = make_manifest(vec![]);
        let md = render_md(&m);
        assert!(md.starts_with("# NiTriTe"));
        assert!(md.contains("2026-01-01_12-00-00"));
        assert!(md.contains("0 éléments"));
    }

    #[test]
    fn render_md_groups_by_category() {
        let m = make_manifest(vec![
            ("Apps", "Systeme", "app data"),
            ("Pilotes", "Systeme", "driver data"),
            ("Favoris", "Navigateurs", "bookmarks"),
        ]);
        let md = render_md(&m);
        // Each category appears once as h2
        let systeme_count = md.matches("## Systeme").count();
        let nav_count = md.matches("## Navigateurs").count();
        assert_eq!(systeme_count, 1);
        assert_eq!(nav_count, 1);
        assert!(md.contains("### Apps"));
        assert!(md.contains("### Pilotes"));
        assert!(md.contains("### Favoris"));
    }

    #[test]
    fn render_txt_contains_header_and_items() {
        let m = make_manifest(vec![("Test Item", "Categorie", "some data")]);
        let txt = render_txt(&m);
        assert!(txt.contains("NiTriTe"));
        assert!(txt.contains("Test Item"));
        assert!(txt.contains("some data"));
    }

    #[test]
    fn render_html_escapes_xss_in_data() {
        let m = make_manifest(vec![
            ("Item", "Systeme", "<script>alert('xss')</script>"),
        ]);
        let html = render_html(&m);
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn render_html_has_nav_and_sections() {
        let m = make_manifest(vec![
            ("App", "Systeme", "data"),
            ("Net", "Reseau", "net data"),
        ]);
        let html = render_html(&m);
        assert!(html.contains("<nav>"));
        assert!(html.contains("Systeme"));
        assert!(html.contains("Reseau"));
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupEntryInfo {
    pub filename: String,
    pub date: String,
    pub size: String,
    pub items_count: usize,
}

