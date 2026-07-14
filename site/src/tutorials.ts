import { initcommon } from "./common";

initcommon();

const filters = Array.from(document.querySelectorAll<HTMLButtonElement>("[data-level]"));
const cards = Array.from(document.querySelectorAll<HTMLElement>("[data-tutorial]"));
const empty = document.querySelector<HTMLElement>("[data-emptyfilter]");

const filterlevel = (level: string) => {
  let visible = 0;
  filters.forEach((button) => {
    const active = button.dataset.level === level;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  });
  cards.forEach((card) => {
    const match = level === "all" || card.dataset.tutorial === level;
    card.hidden = !match;
    if (match) visible += 1;
  });
  if (empty) empty.hidden = visible > 0;
};

filters.forEach((button) => {
  button.addEventListener("click", () => filterlevel(button.dataset.level ?? "all"));
});

const revealhash = () => {
  if (!window.location.hash) return;
  const target = document.querySelector<HTMLElement>(window.location.hash);
  if (!target) return;
  const level = target.dataset.lesson;
  if (level) filterlevel(level);
};

window.addEventListener("hashchange", revealhash);
revealhash();
