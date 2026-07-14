import * as three from "three";

type Signal = {
  curve: three.Curve<three.Vector3>;
  mesh: three.Mesh;
  offset: number;
  speed: number;
};

const color = {
  cyan: 0x33d6ff,
  amber: 0xffb347,
  mint: 0x80f0c0,
  coral: 0xff5a6f,
  graphite: 0x182630,
  paper: 0xdce8f2,
};

const labeltexture = (text: string, tint: string) => {
  const canvas = document.createElement("canvas");
  canvas.width = 320;
  canvas.height = 72;
  const context = canvas.getContext("2d");
  if (!context) return null;

  context.clearRect(0, 0, canvas.width, canvas.height);
  context.fillStyle = "rgba(5, 7, 11, .88)";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.strokeStyle = tint;
  context.lineWidth = 2;
  context.strokeRect(1, 1, canvas.width - 2, canvas.height - 2);
  context.fillStyle = "#dce8f2";
  context.font = "500 24px monospace";
  context.textAlign = "center";
  context.textBaseline = "middle";
  context.fillText(text.toUpperCase(), canvas.width / 2, canvas.height / 2);

  const texture = new three.CanvasTexture(canvas);
  texture.colorSpace = three.SRGBColorSpace;
  return texture;
};

const addlabel = (group: three.Group, text: string, tint: string) => {
  const map = labeltexture(text, tint);
  if (!map) return;
  const material = new three.SpriteMaterial({
    map,
    transparent: true,
    opacity: 0.88,
    depthWrite: false,
  });
  const sprite = new three.Sprite(material);
  sprite.position.set(0, -1.05, 0);
  sprite.scale.set(2.4, 0.54, 1);
  group.add(sprite);
};

const addnode = (
  graph: three.Group,
  position: three.Vector3,
  tint: number,
  label: string,
  scale = 1,
) => {
  const group = new three.Group();
  group.position.copy(position);
  group.scale.setScalar(scale);

  const shell = new three.Mesh(
    new three.OctahedronGeometry(0.58, 0),
    new three.MeshStandardMaterial({
      color: tint,
      emissive: tint,
      emissiveIntensity: 0.45,
      metalness: 0.75,
      roughness: 0.24,
      wireframe: true,
    }),
  );
  const core = new three.Mesh(
    new three.OctahedronGeometry(0.25, 0),
    new three.MeshBasicMaterial({ color: tint, transparent: true, opacity: 0.84 }),
  );
  const ring = new three.Mesh(
    new three.TorusGeometry(0.8, 0.016, 6, 40),
    new three.MeshBasicMaterial({ color: tint, transparent: true, opacity: 0.52 }),
  );
  ring.rotation.x = Math.PI / 2;
  group.add(shell, core, ring);
  addlabel(group, label, "#" + tint.toString(16).padStart(6, "0"));
  graph.add(group);
  return group;
};

const route = (
  graph: three.Group,
  from: three.Vector3,
  to: three.Vector3,
  tint: number,
  bend: number,
  index: number,
) => {
  const middle = from.clone().lerp(to, 0.5);
  middle.z += bend;
  middle.y += Math.sin(index * 1.7) * 0.24;
  const curve = new three.QuadraticBezierCurve3(from.clone(), middle, to.clone());
  const points = curve.getPoints(64);
  const geometry = new three.BufferGeometry().setFromPoints(points);
  const material = new three.LineBasicMaterial({
    color: tint,
    transparent: true,
    opacity: 0.34,
  });
  graph.add(new three.Line(geometry, material));

  const pulse = new three.Mesh(
    new three.SphereGeometry(0.075, 10, 10),
    new three.MeshBasicMaterial({ color: tint }),
  );
  graph.add(pulse);
  return {
    curve,
    mesh: pulse,
    offset: (index * 0.137) % 1,
    speed: 0.055 + (index % 3) * 0.009,
  };
};

export const startscene = (canvas: HTMLCanvasElement) => {
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  let renderer: three.WebGLRenderer;

  try {
    renderer = new three.WebGLRenderer({
      canvas,
      antialias: true,
      alpha: false,
      powerPreference: "high-performance",
    });
  } catch {
    canvas.dataset.state = "unavailable";
    return () => {};
  }

  renderer.setClearColor(0x020407, 1);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 1.75));
  renderer.outputColorSpace = three.SRGBColorSpace;
  renderer.toneMapping = three.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.15;

  const scene = new three.Scene();
  scene.fog = new three.FogExp2(0x020407, 0.055);
  const camera = new three.PerspectiveCamera(44, 1, 0.1, 80);
  camera.position.set(0, 3.4, 14.5);
  const focus = new three.Vector3(0, -0.1, 0);

  scene.add(new three.AmbientLight(0x77a8be, 1.2));
  const keylight = new three.PointLight(color.cyan, 38, 22);
  keylight.position.set(2, 5, 6);
  scene.add(keylight);
  const warmlight = new three.PointLight(color.amber, 24, 16);
  warmlight.position.set(6, -3, 3);
  scene.add(warmlight);

  const grid = new three.GridHelper(32, 32, color.cyan, color.graphite);
  grid.position.y = -2.75;
  const gridmaterial = grid.material as three.Material;
  gridmaterial.transparent = true;
  gridmaterial.opacity = 0.22;
  scene.add(grid);

  const graph = new three.Group();
  graph.position.set(2.3, 0.15, 0);
  scene.add(graph);

  const task = new three.Vector3(-5.2, 0, 0);
  const agents = [
    new three.Vector3(-1.6, 2.05, -0.6),
    new three.Vector3(-1.1, 0.7, 0.9),
    new three.Vector3(-1.1, -0.7, -0.9),
    new three.Vector3(-1.6, -2.05, 0.6),
  ];
  const checks = [
    new three.Vector3(2.5, 1.15, 0.55),
    new three.Vector3(2.5, -1.15, -0.55),
  ];
  const merge = new three.Vector3(5.8, 0, 0);

  const nodes = [
    addnode(graph, task, color.paper, "task", 1.08),
    ...agents.map((position, index) =>
      addnode(graph, position, color.cyan, "agent 0" + (index + 1), 0.86),
    ),
    ...checks.map((position, index) =>
      addnode(graph, position, color.amber, "check 0" + (index + 1), 0.9),
    ),
    addnode(graph, merge, color.mint, "selected", 1.12),
  ];

  const signals: Signal[] = [];
  agents.forEach((agent, index) => {
    signals.push(route(graph, task, agent, color.cyan, index % 2 ? 0.7 : -0.7, index));
    const check = checks[index % checks.length];
    signals.push(route(graph, agent, check, color.amber, index % 2 ? -0.5 : 0.5, index + 4));
  });
  checks.forEach((check, index) => {
    signals.push(route(graph, check, merge, color.mint, index ? 0.45 : -0.45, index + 8));
  });

  const pointer = new three.Vector2();
  const target = new three.Vector2();
  let running = true;
  let elapsed = 0;
  let last = performance.now();

  const resize = () => {
    const width = Math.max(canvas.clientWidth, 1);
    const height = Math.max(canvas.clientHeight, 1);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.fov = width < 700 ? 54 : 44;
    if (width < 700) {
      graph.scale.setScalar(0.48);
      graph.position.set(0, -7, 0);
      grid.position.y = -6.5;
      focus.set(0, -1.8, 0);
    } else if (width < 1000) {
      graph.scale.setScalar(0.62);
      graph.position.set(3.2, -0.5, 0);
      grid.position.y = -2.75;
      focus.set(0, -0.4, 0);
    } else {
      graph.scale.setScalar(0.68);
      graph.position.set(4.8, 0.05, 0);
      grid.position.y = -2.75;
      focus.set(0, -0.1, 0);
    }
    camera.updateProjectionMatrix();
  };

  const draw = (now: number) => {
    const delta = Math.min((now - last) / 1000, 0.05);
    last = now;
    if (running && !reduce) elapsed += delta;

    pointer.lerp(target, 0.035);
    camera.position.x = pointer.x * 0.35;
    camera.position.y = 3.4 + pointer.y * 0.35;
    camera.lookAt(focus);

    nodes.forEach((node, index) => {
      node.rotation.y = elapsed * (0.18 + index * 0.008);
      node.rotation.x = Math.sin(elapsed * 0.45 + index) * 0.08;
      const ring = node.children[2];
      ring.rotation.z = elapsed * (index % 2 ? 0.2 : -0.2);
    });

    signals.forEach((signal) => {
      const progress = (signal.offset + elapsed * signal.speed) % 1;
      signal.mesh.position.copy(signal.curve.getPointAt(progress));
      const visibility = Math.sin(progress * Math.PI);
      signal.mesh.scale.setScalar(0.65 + visibility * 0.8);
    });

    renderer.render(scene, camera);
    canvas.dataset.ready = "true";
  };

  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  resize();

  const visibility = new IntersectionObserver((entries) => {
    running = entries.some((entry) => entry.isIntersecting) && !document.hidden;
  });
  visibility.observe(canvas);

  const onpointer = (event: PointerEvent) => {
    const bounds = canvas.getBoundingClientRect();
    target.x = ((event.clientX - bounds.left) / bounds.width - 0.5) * 2;
    target.y = -((event.clientY - bounds.top) / bounds.height - 0.5) * 2;
  };
  canvas.addEventListener("pointermove", onpointer, { passive: true });
  canvas.addEventListener("pointerleave", () => target.set(0, 0), { passive: true });
  document.addEventListener("visibilitychange", () => {
    running = !document.hidden;
  });

  if (reduce) {
    draw(performance.now());
  } else {
    renderer.setAnimationLoop(draw);
  }

  return () => {
    observer.disconnect();
    visibility.disconnect();
    renderer.setAnimationLoop(null);
    renderer.dispose();
  };
};
