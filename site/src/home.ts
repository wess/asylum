import { initcommon } from "./common";
import { startscene } from "./scene";

initcommon();

const canvas = document.querySelector<HTMLCanvasElement>("#agentgrid");
const stopscene = canvas ? startscene(canvas) : () => {};

const stages = Array.from(document.querySelectorAll<HTMLElement>(".flowline li"));
let stage = 0;
const setstage = () => {
  stages.forEach((item, index) => item.classList.toggle("active", index === stage));
  stage = (stage + 1) % stages.length;
};
setstage();
const stageclock = window.setInterval(setstage, 1400);

window.addEventListener(
  "pagehide",
  () => {
    window.clearInterval(stageclock);
    stopscene();
  },
  { once: true },
);
