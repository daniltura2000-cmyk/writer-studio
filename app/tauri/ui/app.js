// Writer Studio UI — vanilla JS поверх Tauri invoke-команд (каркас M2.5/M3.5).
// У Tauri-вікні: window.__TAURI__.core.invoke; у звичайному браузері — заглушка-попередження.

const invoke = window.__TAURI__?.core?.invoke;
const $ = (id) => document.getElementById(id);
let root = localStorage.getItem("skri.root") || "";

if (!invoke) {
  document.body.insertAdjacentHTML("afterbegin",
    '<div style="padding:10px 32px;background:#fef3c7;color:#92400e;font-size:13px">' +
    "Це UI каркас: запускайте через Tauri (npm run tauri dev / cargo run), інакше invoke-команди недоступні.</div>");
}

function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

async function cmd(name, args = {}) {
  if (!invoke) { alert("Запустіть додаток через Tauri — invoke недоступний у браузері."); throw new Error("no invoke"); }
  try {
    return await invoke(name, { root: root || undefined, ...args });
  } catch (e) {
    alert("Помилка команди " + name + ":\n" + e);
    throw e;
  }
}

// --- вкладки ---
document.querySelectorAll("nav .tab").forEach((t) => {
  t.onclick = () => {
    document.querySelectorAll("nav .tab").forEach((x) => x.classList.remove("active"));
    document.querySelectorAll(".screen").forEach((x) => x.classList.remove("active"));
    t.classList.add("active");
    $(t.dataset.tab).classList.add("active");
  };
});

// --- проєкт ---
$("projRoot").value = root;
$("openProj").onclick = async () => {
  root = $("projRoot").value.trim();
  localStorage.setItem("skri.root", root);
  const el = $("projState");
  try {
    const s = await cmd("project_info");
    el.textContent = `✔ проєкт відкрито — пресет: ${s.preset ?? "?"}, мова: ${s.language ?? "?"}`;
    el.className = "proj-state ok";
  } catch {
    el.textContent = "✖ не проєкт skri (нема .skri/settings.json)";
    el.className = "proj-state bad";
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

// --- біблія ---
$("btnIndex").onclick = async () => {
  const r = await cmd("run_index");
  const out = $("indexOut");
  out.hidden = false;
  out.textContent = JSON.stringify(r, null, 2);
  await loadCards();
};

async function loadCards() {
  const cards = await cmd("run_bible");
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

// --- алерти ---
$("btnDrift").onclick = async () => {
  const r = await cmd("run_drift");
  $("driftOut").innerHTML =
    r.groups.map((g) => `<div class="group"><b>drift: ${esc(g.canonical)}</b> <span class="kind">name-drift</span>
      <pre>варіанти: ${esc(g.variants.join(" | "))}</pre></div>`).join("") +
    r.alerts.map((a) => `<div class="group"><b>алерт #${a.id}</b> <span class="kind">${esc(a.kind)}</span>
      <pre>${esc(JSON.stringify(a.payload, null, 2))}</pre></div>`).join("") ||
    '<div class="note">Алертів немає — рукопис консистентний.</div>';
};

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
    "</table>" ||
    '<div class="note">Змінних немає — додайте %SET / %CHECK у dialogue/.</div>';
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
    "</table>" ||
    '<div class="note">Bark-ів немає.</div>';
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
