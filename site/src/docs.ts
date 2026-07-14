import { initcommon } from "./common";

initcommon();

const articles = Array.from(document.querySelectorAll<HTMLElement>("[data-article]"));
const links = Array.from(document.querySelectorAll<HTMLAnchorElement>("[data-doclink]"));
const outline = document.querySelector<HTMLElement>("[data-outline]");
const input = document.querySelector<HTMLInputElement>("[data-docsearch]");
const results = document.querySelector<HTMLElement>("[data-docresults]");

const slug = (value: string) =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "")
    .trim();

const hashselection = () => {
  const requested = window.location.hash.slice(1);
  const article = articles.find((candidate) => candidate.id === requested);
  if (article) return { article: article.id, heading: "" };

  const heading = document.getElementById(requested);
  const owner = heading?.closest<HTMLElement>("[data-article]");
  if (owner) return { article: owner.id, heading: requested };

  return { article: "overview", heading: "" };
};

const buildoutline = (article: HTMLElement) => {
  if (!outline) return;
  outline.replaceChildren();
  article.querySelectorAll<HTMLElement>("h2, h3").forEach((heading, index) => {
    const id = heading.id || article.id + slug(heading.textContent || String(index));
    heading.id = id;
    const link = document.createElement("a");
    link.href = "#" + id;
    link.textContent = heading.textContent;
    if (heading.tagName === "H3") link.classList.add("nested");
    outline.append(link);
  });
};

const showarticle = (id: string, scroll = false) => {
  const selected = articles.find((article) => article.id === id) ?? articles[0];
  if (!selected) return;

  articles.forEach((article) => article.classList.toggle("active", article === selected));
  links.forEach((link) => {
    const active = link.dataset.doclink === selected.id;
    link.classList.toggle("active", active);
    if (active) link.setAttribute("aria-current", "page");
    else link.removeAttribute("aria-current");
  });
  buildoutline(selected);
  document.title = (selected.dataset.title ?? "Documentation") + " — Asylum";
  if (scroll) window.scrollTo({ top: 0, behavior: "smooth" });
};

const openarticle = (id: string) => {
  window.history.pushState(null, "", "#" + id);
  showarticle(id, true);
};

document.addEventListener("click", (event) => {
  const link = (event.target as HTMLElement).closest<HTMLAnchorElement>("a[href^='#']");
  if (!link) return;
  const id = link.hash.slice(1);
  if (!articles.some((article) => article.id === id)) return;
  event.preventDefault();
  openarticle(id);
});

const synchash = () => {
  const selection = hashselection();
  showarticle(selection.article);
  if (selection.heading) {
    window.requestAnimationFrame(() => {
      document.getElementById(selection.heading)?.scrollIntoView({ block: "start" });
    });
  } else {
    window.requestAnimationFrame(() => window.scrollTo({ top: 0, behavior: "auto" }));
  }
};

window.addEventListener("hashchange", synchash);
window.addEventListener("popstate", synchash);

const search = (query: string) => {
  if (!results) return;
  const normalized = query.trim().toLowerCase();
  results.replaceChildren();
  results.hidden = normalized.length < 2;
  if (normalized.length < 2) return;

  const matches = articles
    .map((article) => {
      const title = article.dataset.title ?? article.id;
      const source = [title, article.dataset.keywords, article.textContent].join(" ").toLowerCase();
      const score =
        (title.toLowerCase().includes(normalized) ? 3 : 0) +
        (article.dataset.keywords?.includes(normalized) ? 2 : 0) +
        (source.includes(normalized) ? 1 : 0);
      return { article, title, score };
    })
    .filter((match) => match.score > 0)
    .sort((a, b) => b.score - a.score)
    .slice(0, 7);

  if (matches.length === 0) {
    const empty = document.createElement("p");
    empty.textContent = "No matching documentation";
    results.append(empty);
    return;
  }

  matches.forEach(({ article, title }) => {
    const link = document.createElement("a");
    link.href = "#" + article.id;
    link.textContent = title;
    link.addEventListener("click", () => {
      if (input) input.value = "";
      results.hidden = true;
    });
    results.append(link);
  });
};

input?.addEventListener("input", () => search(input.value));
input?.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    input.value = "";
    search("");
    input.blur();
  }
});

document.addEventListener("keydown", (event) => {
  const target = event.target as HTMLElement;
  if (event.key === "/" && !["INPUT", "TEXTAREA"].includes(target.tagName)) {
    event.preventDefault();
    input?.focus();
  }
});

synchash();
if (document.readyState !== "complete") {
  window.addEventListener("load", synchash, { once: true });
}
