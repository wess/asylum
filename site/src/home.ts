import { initcommon } from "./common";

initcommon();

const reducemotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const savedata = (navigator as unknown as { connection?: { saveData?: boolean } }).connection
  ?.saveData === true;

const canvas = document.querySelector<HTMLCanvasElement>("#agentgrid");
let stopscene = () => {};

// Load the heavy Three.js hero scene lazily and only when it will actually be
// used: skip it entirely under reduced-motion or data-saver, and otherwise defer
// the dynamic import until the hero canvas scrolls into view. This keeps Three.js
// out of the initial bundle (the page is meaningful without it).
if (canvas && !reducemotion && !savedata) {
  const load = () => {
    import("./scene")
      .then(({ startscene }) => {
        stopscene = startscene(canvas);
      })
      .catch(() => {});
  };
  if ("IntersectionObserver" in window) {
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        observer.disconnect();
        load();
      }
    });
    observer.observe(canvas);
  } else {
    load();
  }
}

const stages = Array.from(document.querySelectorAll<HTMLElement>(".flowline li"));
let stage = 0;
const setstage = () => {
  stages.forEach((item, index) => item.classList.toggle("active", index === stage));
  stage = (stage + 1) % stages.length;
};
// Always show the first step. Under reduced motion, leave it stable and do not
// start the auto-cycling clock (no repeating visual state change).
setstage();
const stageclock = reducemotion ? 0 : window.setInterval(setstage, 1400);

window.addEventListener(
  "pagehide",
  () => {
    if (stageclock) window.clearInterval(stageclock);
    stopscene();
  },
  { once: true },
);
