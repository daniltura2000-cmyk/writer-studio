// Writer Studio UI — vanilla JS поверх Tauri invoke-команд (редактор, M-редактор).
// У Tauri-вікні: window.__TAURI__.core.invoke; у звичайному браузері — заглушка-попередження.

const invoke = window.__TAURI__?.core?.invoke;
const $ = (id) => document.getElementById(id);
let root = localStorage.getItem("skri.root") || "";

if (!invoke) {
  document.body.insertAdjacentHTML("afterbegin",
    '<div style="padding:10px 32px;background:#3a2e12;color:#fbbf24;font-size:13px">' +
    "Це UI каркас: запускайте через Tauri (cargo run / tauri dev), інакше invoke-команди недоступні.</div>");
}

function esc(s) {
  return String(s).replace(/[&<>\"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

async function cmd(name, args = {}) {
  const r = await cmdq(name, args);
  if (r.err != null) { alert("Помилка команди " + name + ":\n" + r.err); throw new Error(r.err); }
  return r.ok;
}

// тихий варіант: не показує alert (для фонових перевірок при запуску)
async function cmdq(name, args = {}) {
  if (!invoke) return { err: "no invoke" };
  try {
    return { ok: await invoke(name, { root: root || undefined, ...args }) };
  } catch (e) {
    return { err: String(e) };
  }
}

// --- верхні вкладки ---
document.querySelectorAll("nav .tab").forEach((t) => {
  t.onclick = () => {
    document.querySelectorAll("nav .tab").forEach((x) => x.classList.remove("active"));
    document.querySelectorAll(".screen").forEach((x) => x.classList.remove("active"));
    t.classList.add("active");
    $(t.dataset.tab).classList.add("active");
  };
});

// --- проєкт (шапка) ---
function setProjState(ok, text) {
  const el = $("projState");
  el.textContent = text;
  el.className = "proj-state " + (ok ? "ok" : "bad");
}

async function openProject(path, quiet = false) {
  root = path;
  localStorage.setItem("skri.root", root);
  $("projRoot").value = root;
  $("setupRoot").value = root;
  const r = await cmdq("project_info");
  if (r.err != null) {
    setProjState(false, "✖ не проєкт skri (нема .skri/settings.json)");
    return false;
  }
  const s = r.ok;
  setProjState(true, `✔ проєкт відкрито — пресет: ${s.preset ?? "?"}, мова: ${s.language ?? "?"}`);
  $("setup").classList.add("hidden");
  await loadDocs();
  return true;
}

$("openProj").onclick = async () => {
  const ok = await openProject($("projRoot").value.trim());
  if (!ok) { $("setupMsg").textContent = "Ця папка — не проєкт skri. Можете створити новий нижче."; showSetup(false); }
};

// --- вибір проєкту/пресету при запуску (екран-оверлей) ---
function showSetup(fillRoot = true) {
  if (fillRoot && !$("setupRoot").value) $("setupRoot").value = root;
  $("setup").classList.remove("hidden");
}

async function fillPresets() {
  try {
    const presets = await cmd("list_presets");
    $("setupPreset").innerHTML = presets
      .map((p) => `<option value="${esc(p.id)}">${esc(p.id)} — ${esc(p.desc)}</option>`)
      .join("");
  } catch { /* без інтернету/інvoke — поле просто порожнє */ }
}

$("setupOpen").onclick = async () => {
  const p = $("setupRoot").value.trim();
  if (!p) { $("setupMsg").textContent = "Вкажіть шлях до папки проєкту."; return; }
  const ok = await openProject(p);
  if (!ok) $("setupMsg").textContent = "Не проєкт skri (нема .skri/settings.json) — або виправте шлях, або створіть проєкт.";
};

$("setupCreate").onclick = async () => {
  const p = $("setupRoot").value.trim();
  if (!p) { $("setupMsg").textContent = "Вкажіть шлях до НОВОЇ папки проєкту."; return; }
  try {
    root = p; // бо cmd() підставляє root сам — до створення він ще не був виставлений
    localStorage.setItem("skri.root", root);
    const r = await cmdq("init_project", { preset: $("setupPreset").value });
    if (r.err != null) { $("setupMsg").textContent = "Не вдалося створити: " + r.err; return; }
    $("setupMsg").textContent = "";
    await openProject(p);
  } catch (e) {
    $("setupMsg").textContent = "Не вдалося створити: " + e;
  }
};

// --- онбордінг: напрямки ---
async function loadDirections() {
  const dirs = await cmd("list_directions");
  $("dirs").innerHTML = dirs
    .map(
      (d) => `<button class="dir" data-id="${esc(d.id)}">
        <span class="t">${esc(d.title)}</span>
        <span class="o">${esc(d.outcome)}</span>
        ${d.preset_info ? `<span class="p">${esc(d.preset_info)}</span>` : ""}
      </button>`
    )
    .join("");
}
loadDirections();

// --- моделі: кнопковий каталог (як у model-picker.html) ---
let catalog = [];
let currentTask = null;

async function loadCatalog() {
  catalog = await cmd("model_catalog");
  $("tasks").innerHTML = "";
  catalog.forEach((t) => {
    const b = document.createElement("button");
    b.className = "task";
    b.innerHTML = `<span class="t">${esc(t.label)}</span><span class="h">${esc(t.hint)}</span>`;
    b.onclick = () => selectTask(t);
    t._btn = b;
    $("tasks").appendChild(b);
  });
}
loadCatalog();

function selectTask(t) {
  currentTask = t;
  catalog.forEach((x) => x._btn.classList.remove("active"));
  t._btn.classList.add("active");
  $("panel").innerHTML =
    `<h2>${esc(t.label)}</h2><div class="hint">${esc(t.hint)}</div>` +
    t.options
      .map(
        (o, j) => `<label class="opt">
        <input type="radio" name="m" ${j === 0 ? "checked" : ""}>
        <span><span class="l">${esc(o.label)}</span>
        <span class="badge ${o.local ? "b-local" : "b-cloud"}">${o.local ? "локальна" : "хмара (BYOK)"}</span>
        ${j === 0 ? '<span class="badge b-auto">автоматично</span>' : ""}
        <div class="n">${esc(o.note)}</div></span>
      </label>`
      )
      .join("");
  $("panel").querySelectorAll("input").forEach((i) => (i.onchange = applyModel));
  applyModel();
}

async function applyModel() {
  const inputs = [...$("panel").querySelectorAll("input")];
  const idx = inputs.findIndex((i) => i.checked);
  if (idx < 0 || !currentTask) return;
  const o = currentTask.options[idx];
  const auto = idx === 0;
  $("modelOut").textContent = JSON.stringify(
    { models: { [currentTask.id]: { model: o.id, auto } } },
    null, 2
  );
  if (root) await cmd("set_model", { task: currentTask.id, model: o.id, auto });
}

// ================= СТУДІЯ: сайдбар-таби =================
document.querySelectorAll(".stab").forEach((t) => {
  t.onclick = () => {
    document.querySelectorAll(".stab").forEach((x) => x.classList.remove("active"));
    document.querySelectorAll(".stabpane").forEach((x) => x.classList.remove("active"));
    t.classList.add("active");
    $("stab-" + t.dataset.stab).classList.add("active");
    if (t.dataset.stab === "bible" && !$("cards").dataset.loaded) loadBible();
    if (t.dataset.stab === "alerts" && !lastAlerts.length) loadAlerts();
    if (t.dataset.stab === "release" && !$("kanban").dataset.loaded) loadRelease();
  };
});

// --- таб «Глави» + редактор ---
let currentDoc = null;
let dirty = false;

async function loadDocs() {
  if (!root) return;
  const docs = await cmd("list_docs");
  $("docList").innerHTML = docs
    .map(
      (d) => `<button class="doc ${currentDoc && currentDoc.path === d.path ? "sel" : ""}" data-path="${esc(d.path)}">
        <span class="t">${esc(d.title)}</span>
        <span class="m">${d.word_count} слів</span>
      </button>`
    )
    .join("") || '<div class="note">У manuscript/ немає .md-файлів.</div>';
  $("docList").querySelectorAll(".doc").forEach((b) => {
    b.onclick = () => openDoc(b.dataset.path);
  });
}
$("btnReloadDocs").onclick = loadDocs;

async function openDoc(path) {
  if (dirty && !confirm("Є незбережені зміни. Перейти без збереження?")) return;
  const d = await cmd("read_doc", { rel: path });
  currentDoc = d;
  dirty = false;
  $("editor").value = d.body;
  $("docTitle").textContent = d.title;
  $("docMeta").textContent = d.has_frontmatter
    ? "frontmatter: " + d.meta.map(([k, v]) => `${k}=${v}`).join(", ")
    : "без frontmatter";
  $("saveState").textContent = "";
  updateCounts();
  document.querySelectorAll(".doc").forEach((b) =>
    b.classList.toggle("sel", b.dataset.path === path));
}

const editor = $("editor");
editor.addEventListener("input", () => { dirty = true; updateCounts(); });

function updateCounts() {
  const body = editor.value;
  const words = (body.match(/\S+/g) || []).length;
  $("cntWords").textContent = `${words} слів · ${body.length} символів`;
}

async function saveDoc() {
  if (!currentDoc) { $("saveState").textContent = "Немає відкритої глави."; return; }
  try {
    await cmd("write_doc", { rel: currentDoc.path, body: editor.value });
    dirty = false;
    const t = new Date();
    $("saveState").textContent = `✔ збережено ${String(t.getHours()).padStart(2, "0")}:${String(t.getMinutes()).padStart(2, "0")}`;
    await loadDocs(); // свіжі word_count у списку
  } catch { /* помилку вже показано в cmd() */ }
}
$("btnSave").onclick = saveDoc;

// Ctrl+S — збереження (глобально, перехоплюємо браузерний діалог)
window.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && (e.key === "s" || e.key === "S")) {
    e.preventDefault();
    if (document.activeElement === editor || currentDoc) saveDoc();
  }
});

// --- таб «Біблія»: картки сутностей ---
async function loadBible() {
  const cards = await cmd("run_bible");
  $("cards").dataset.loaded = "1";
  $("cards").innerHTML = cards
    .map(
      (c) => `<div class="card">
      <span class="name">${esc(c.canonical)}</span><span class="type">${esc(c.entity_type)}</span>
      <div class="meta">${c.mention_total} згадок${c.aliases.length ? " · варіанти: " + esc(c.aliases.join(", ")) : ""}</div>
      ${c.docs.length ? `<div class="meta">де: ${c.docs.map((d) => `${esc(d.path)}×${d.mentions}`).join(", ")}</div>` : ""}
      ${
        c.assertions.length
          ? "<ul>" + c.assertions.map((a) => `<li>${esc(a.attribute)}: ${esc(a.value)} <span class="al">(${esc(a.doc)})</span></li>`).join("") + "</ul>"
          : ""
      }
    </div>`
    )
    .join("") || '<div class="note">Сутностей не знайдено — покладіть глави у manuscript/ і натисніть «Реіндексувати».</div>';
}

$("btnIndex").onclick = async () => {
  const r = await cmd("run_index");
  const out = $("indexOut");
  out.hidden = false;
  out.textContent = JSON.stringify(r, null, 2);
  await loadBible();
};

// --- таб «Алерти»: усі алери + фільтр по kind ---
const KIND_LABELS = {
  "name-drift": "дрейф імен",
  "assertion-conflict": "конфлікт канону",
  "stat-change": "ретроактивна правка статів",
  "stat-error": "помилка статів",
  "timeline-backwards": "стрибок назад у часі",
  "location-conflict": "конфлікт локацій",
  "crosspost-stale": "змінено після публікації",
  "episode-check": "зауваження епізоду",
  "ink-external-change": "зовнішня зміна ink/yarn",
};

let lastAlerts = [];

function renderAlerts() {
  const f = $("alertFilter").value;
  const list = f ? lastAlerts.filter((a) => a.kind === f) : lastAlerts;
  $("alertList").innerHTML =
    list
      .map(
        (a) => `<div class="group"><b>алерт #${a.id}</b> <span class="kind">${esc(KIND_LABELS[a.kind] || a.kind)}</span>
      <pre>${esc(JSON.stringify(a.payload, null, 2))}</pre></div>`
      )
      .join("") || '<div class="note">Алертів немає — рукопис консистентний.</div>';
  // бейдж на вкладці
  const badge = $("alertBadge");
  badge.hidden = lastAlerts.length === 0;
  badge.textContent = lastAlerts.length;
  // опції фільтра — kind, що є в даних
  const kinds = [...new Set(lastAlerts.map((a) => a.kind))];
  $("alertFilter").innerHTML =
    '<option value="">усі kind</option>' +
    kinds.map((k) => `<option value="${esc(k)}" ${k === f ? "selected" : ""}>${esc(KIND_LABELS[k] || k)} (${lastAlerts.filter((a) => a.kind === k).length})</option>`).join("");
}

function fillAlerts(alerts, summary) {
  lastAlerts = alerts;
  $("alertsSummary").textContent = summary;
  renderAlerts();
}

async function loadAlerts() {
  const r = await cmd("run_drift");
  fillAlerts(
    r.alerts,
    `drift-груп: ${r.groups.length} · алертів: ${r.alerts.length}` +
      (r.groups.length ? "" : " — натисніть «Перевірити» для повного прогону валідаторів")
  );
  // drift-групи показуємо як віртуальні алерти зверху списку
  if (r.groups.length) {
    $("alertList").innerHTML =
      r.groups
        .map(
          (g) => `<div class="group"><b>drift: ${esc(g.canonical)}</b> <span class="kind">name-drift</span>
        <pre>варіанти: ${esc(g.variants.join(" | "))}</pre></div>`
        )
        .join("") + $("alertList").innerHTML;
  }
}
$("alertFilter").onchange = renderAlerts;

// --- кнопка «Перевірити»: повний конвеєр валідаторів ---
$("btnCheck").onclick = async () => {
  if (!root) { showSetup(); return; }
  const r = await cmd("run_checks");
  const sum = `документів: ${r.docs} · слів: ${r.words} · сутностей: ${r.entities} · тверджень: ${r.assertions} · ` +
    `конфліктів канону: ${r.conflicts} · злиттів: ${r.merged} · drift-груп: ${r.drift_groups}`;
  fillAlerts(r.alerts, sum + (r.timeline_missing.length ? ` · глав без time-маркера: ${r.timeline_missing.length}` : ""));
  // голови без маркерів — попередження (не алерт)
  if (r.timeline_missing.length) {
    $("alertList").insertAdjacentHTML(
      "afterbegin",
      `<div class="group warnbox"><b>⚠ глави без time-маркерів</b>
       <pre>${esc(r.timeline_missing.map(([p, n]) => `${p} (розділ ${n})`).join("\n"))}</pre></div>`
    );
  }
  await loadBible(); // картки теж оновилися після конвеєра
  // перемкнутись на таб «Алерти»
  document.querySelector('.stab[data-stab="alerts"]').click();
};

// --- інструменти діалогів (як раніше) ---
$("btnVars").onclick = async () => {
  const rows = await cmd("var_audit");
  $("varsOut").innerHTML =
    `<table class="t"><tr><th>змінна</th><th>set</th><th>check</th><th>unset</th><th>статус</th></tr>` +
    rows
      .map(
        (v) =>
          `<tr><td>${esc(v.var_name)}</td><td>${v.sets.map(esc).join(", ") || "—"}</td>` +
          `<td>${v.checks.map(esc).join(", ") || "—"}</td><td>${v.unsets.map(esc).join(", ") || "—"}</td>` +
          `<td>${v.orphan_check ? '<span class="warn">сирота (check без set)</span>' : v.dead_flag ? '<span class="warn">мертвий прапорець</span>' : "✔"}</td></tr>`
      )
      .join("") +
    "</table>";
};

$("btnBarks").onclick = async () => {
  const rows = await cmd("barks");
  $("barksOut").innerHTML =
    `<table class="t"><tr><th>тригер</th><th>пріоритет</th><th>cooldown</th><th>умови</th><th>варіанти</th></tr>` +
    rows
      .map(
        (b) =>
          `<tr><td>${esc(b.trigger)}</td><td>${b.priority}</td><td>${b.cooldown}s</td>` +
          `<td><pre style="margin:0">${esc(b.conditions)}</pre></td><td>${esc(b.variants.join(" / "))}</td></tr>`
      )
      .join("") +
    "</table>";
};

$("btnEpisode").onclick = async () => {
  const rel = $("epPath").value.trim();
  if (!rel) { alert("Вкажіть шлях епізоду, напр. manuscript/ep01.md"); return; }
  const r = await cmd("episode_validate", { rel });
  const c = r.check;
  $("epOut").innerHTML =
    `<div class="group"><b>Валідація епізоду ${esc(rel)}</b>
      <pre>${c.passed ? "✔ структурно пройшов" : "✖ є зауваження"}
cold open: ${esc(c.cold_open ?? "— немає")}
turns: ${c.turns}
hook: ${esc(c.hook ?? "— немає")}</pre>
      ${c.issues.map((i) => `<div class="warn">• ${esc(i)}</div>`).join("")}
      ${r.rule_hits.length ? r.rule_hits.map((i) => `<div class="warn">retcon?: ${esc(i)}</div>`).join("") : ""}
    </div>
    <table class="t"><tr><th>сцена</th><th>action</th><th>діалог</th><th>visual_ratio</th><th>вердикт</th></tr>` +
    r.mute
      .map(
        (m) =>
          `<tr><td>${esc(m.scene)}</td><td>${m.action_lines}</td><td>${m.dialogue_lines}</td>` +
          `<td>${m.visual_ratio}</td><td>${esc(m.verdict)}</td></tr>`
      )
      .join("") +
    "</table>";
};

// --- таб «Реліз»: kanban + прогноз буфера ---
const STATUSES = ["draft", "edit", "buffer", "scheduled", "published"];

async function loadRelease() {
  const b = await cmd("release_board");
  $("kanban").dataset.loaded = "1";
  const fc = b.forecast;
  $("forecastBox").innerHTML =
    `<b>Прогноз буфера</b> (вікно ${fc.window} глав, вибірка ${fc.samples})` +
    `<pre>швидкість: ${fc.words_per_day.toFixed(1)} слів/день
буфер: ${fc.buffer_chapters} глав · ${fc.buffer_words} слів
днів на главу: ${fc.days_per_chapter.toFixed(1)}` +
    (fc.days_left != null ? `\nзапас вичерпається через ~${fc.days_left.toFixed(1)} днів` : "\nшвидкість невідома (мало глав)") +
    (fc.exhaustion_date ? `\nзакінчиться ${esc(fc.exhaustion_date)}` : "") +
    `</pre>`;
  $("kanban").innerHTML = b.columns
    .map((col) => {
      const idx = STATUSES.indexOf(col.status);
      const next = idx >= 0 && idx < STATUSES.length - 1 ? STATUSES[idx + 1] : null;
      return `<div class="kcol">
        <div class="khead">${esc(col.status)} <span class="kn">${col.chapters.length}</span></div>
        ${col.chapters
          .map(
            (c) => `<div class="kcard">
          <span class="t" title="${esc(c.doc_path)}">${esc(c.title)}</span>
          <span class="m">${c.word_count} слів${c.pub_date ? " · " + esc(new Date(c.pub_date * 1000).toISOString().slice(0, 10)) : ""}</span>
          ${next ? `<button class="act mini" data-doc="${esc(c.doc_path)}" data-next="${next}">→ ${next}</button>` : '<span class="done">✔ опубліковано</span>'}
        </div>`
          )
          .join("") || '<div class="note">—</div>'}
      </div>`;
    })
    .join("");
  $("kanban").querySelectorAll("button.mini").forEach((btn) => {
    btn.onclick = async () => {
      await cmd("release_set_status", { rel: btn.dataset.doc, status: btn.dataset.next });
      await loadRelease();
    };
  });
}

// --- запуск: якщо проєкт не заданий/недійсний — екран вибору ---
(async () => {
  await fillPresets();
  if (root) {
    const ok = await openProject(root);
    if (!ok) showSetup(false);
  } else {
    showSetup(false);
  }
})();
