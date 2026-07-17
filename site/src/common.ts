import "@fontsource-variable/unbounded";
import "@fontsource-variable/manrope";
import "@fontsource-variable/jetbrains-mono";
import {
  ArrowDown,
  ArrowRight,
  ArrowUpRight,
  Blocks,
  BookOpenText,
  Check,
  CircleCheck,
  Copy,
  FilePenLine,
  Gauge,
  GitCompareArrows,
  GitFork,
  GraduationCap,
  Info,
  Lightbulb,
  Link,
  ListChecks,
  Menu,
  MessageSquareWarning,
  MessagesSquare,
  MousePointerClick,
  NotebookTabs,
  PackageCheck,
  Play,
  Scale,
  ScanSearch,
  Search,
  ShieldCheck,
  SquareTerminal,
  Terminal,
  Workflow,
  createIcons,
} from "lucide";
import "./styles.css";

const iconset = {
  ArrowDown,
  ArrowRight,
  ArrowUpRight,
  Blocks,
  BookOpenText,
  Check,
  CircleCheck,
  Copy,
  FilePenLine,
  Gauge,
  Github: GitFork,
  GitCompareArrows,
  GitFork,
  GraduationCap,
  Info,
  Lightbulb,
  Link,
  ListChecks,
  Menu,
  MessageSquareWarning,
  MessagesSquare,
  MousePointerClick,
  NotebookTabs,
  PackageCheck,
  Play,
  Scale,
  ScanSearch,
  Search,
  ShieldCheck,
  TerminalSquare: SquareTerminal,
  Terminal,
  Workflow,
};

export const rendericons = () => {
  createIcons({
    attrs: {
      "aria-hidden": "true",
    },
    icons: iconset,
  });
};

const initnavigation = () => {
  const header = document.querySelector<HTMLElement>("[data-header]");
  const nav = document.querySelector<HTMLElement>("[data-nav]");
  const toggle = document.querySelector<HTMLButtonElement>("[data-navtoggle]");
  if (!header || !nav || !toggle) return;

  const close = () => {
    nav.classList.remove("open");
    document.body.classList.remove("navopen");
    toggle.setAttribute("aria-expanded", "false");
    toggle.setAttribute("aria-label", "Open navigation");
  };

  toggle.addEventListener("click", () => {
    const open = !nav.classList.contains("open");
    nav.classList.toggle("open", open);
    document.body.classList.toggle("navopen", open);
    toggle.setAttribute("aria-expanded", String(open));
    toggle.setAttribute("aria-label", open ? "Close navigation" : "Open navigation");
  });

  nav.addEventListener("click", (event) => {
    if ((event.target as HTMLElement).closest("a")) close();
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") close();
  });

  const sync = () => header.classList.toggle("scrolled", window.scrollY > 16);
  sync();
  window.addEventListener("scroll", sync, { passive: true });
};

const initreveal = () => {
  const targets = document.querySelectorAll<HTMLElement>(
    ".sectionhead, .productframe, .capabilitylist article, .learnroute, .tutorialcard",
  );
  targets.forEach((target) => target.setAttribute("data-reveal", ""));

  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    targets.forEach((target) => target.classList.add("revealed"));
    return;
  }

  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        (entry.target as HTMLElement).classList.add("revealed");
        observer.unobserve(entry.target);
      });
    },
    { rootMargin: "0px 0px -8% 0px", threshold: 0.08 },
  );
  targets.forEach((target) => observer.observe(target));
};

const initcopy = () => {
  document.querySelectorAll<HTMLButtonElement>("[data-copy]").forEach((button) => {
    button.addEventListener("click", async () => {
      const block = button.closest(".codeblock");
      const code = block?.querySelector("code")?.textContent ?? "";
      if (!code) return;

      try {
        await navigator.clipboard.writeText(code);
        const original = button.innerHTML;
        button.textContent = "Copied";
        button.classList.add("copied");
        button.setAttribute("aria-label", "Copied");
        window.setTimeout(() => {
          button.innerHTML = original;
          button.classList.remove("copied");
          button.setAttribute("aria-label", "Copy code");
          rendericons();
        }, 1300);
      } catch {
        button.setAttribute("aria-label", "Copy failed");
      }
    });
  });
};

const initvideoposters = () => {
  document.querySelectorAll<HTMLVideoElement>("video").forEach((video) => {
    const source = video.querySelector<HTMLSourceElement>('source[type="video/mp4"]');
    if (!source) return;
    const name = source.src.split("/").pop()?.replace(/\.mp4$/, ".png");
    if (name) video.poster = new URL(`../videos/${name}`, source.src).href;
  });
};

export const initcommon = () => {
  rendericons();
  initnavigation();
  initreveal();
  initcopy();
  initvideoposters();
};
