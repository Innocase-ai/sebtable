function toast(msg) {
  let el = document.getElementById("toast");
  if (!el) {
    el = document.createElement("div");
    el.id = "toast";
    el.setAttribute("role", "status");
    el.setAttribute("aria-live", "polite");
    Object.assign(el.style, {
      position: "fixed", bottom: "16px", left: "50%", transform: "translateX(-50%)",
      background: "#1e2230", color: "#eef0f7", border: "1px solid #2a2e3e",
      padding: "8px 14px", borderRadius: "8px", fontSize: "13px", zIndex: "50"
    });
    document.body.appendChild(el);
  }
  el.textContent = msg;
  el.style.opacity = "1";
  setTimeout(() => { el.style.opacity = "0"; }, 1800);
}

function initCopy() {
  document.querySelectorAll(".copy[data-copy]").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const text = btn.getAttribute("data-copy") || "";
      try {
        await navigator.clipboard.writeText(text);
        toast("Copié : " + text.slice(0, 40));
      } catch {
        toast("Copie impossible");
      }
    });
  });
  const first = document.getElementById("copy-first");
  if (first) {
    first.addEventListener("click", async () => {
      const txt = 'IF({Statut} = "Payé", "✓", "à relancer")';
      try { await navigator.clipboard.writeText(txt); toast("Exemple copié"); } catch { toast(txt); }
    });
  }
}

function initSearch() {
  const input = document.getElementById("search");
  const refTable = document.getElementById("ref-table");
  if (!input) return;
  input.addEventListener("input", () => {
    const q = input.value.trim().toLowerCase();
    document.querySelectorAll(".func").forEach((el) => {
      const name = (el.getAttribute("data-name") || "").toLowerCase();
      const text = el.textContent.toLowerCase();
      const show = !q || name.includes(q) || text.includes(q);
      el.style.display = show ? "" : "none";
    });
    if (refTable) {
      refTable.querySelectorAll("tbody tr").forEach((tr) => {
        const n = (tr.getAttribute("data-name") || "").toLowerCase();
        tr.style.display = !q || n.includes(q) ? "" : "none";
      });
    }
  });
}

function initTocActive() {
  const links = Array.from(document.querySelectorAll(".toc a"));
  const sections = links.map((a) => document.querySelector(a.getAttribute("href"))).filter(Boolean);
  if (!("IntersectionObserver" in window) || sections.length === 0) return;
  const obs = new IntersectionObserver((entries) => {
    entries.forEach((e) => {
      if (e.isIntersecting) {
        links.forEach((a) => a.classList.remove("active"));
        const id = "#" + e.target.id;
        const active = links.find((a) => a.getAttribute("href") === id);
        if (active) active.classList.add("active");
      }
    });
  }, { rootMargin: "-40% 0px -55% 0px", threshold: 0 });
  sections.forEach((s) => obs.observe(s));
}

document.addEventListener("DOMContentLoaded", () => {
  initCopy();
  initSearch();
  initTocActive();
});
