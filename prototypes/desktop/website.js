(() => {
  'use strict';

  const scenarios = {
    fix: {
      name: 'Fix',
      request: 'Can you make search more helpful when there are no results?',
      context: 'Project Aurora · SearchResults.tsx · search empty state, with only the context related to your request.',
      plan: 'Understand search → prepare the empty state → review the clarity of the message.',
      resultTitle: 'A change for you to review',
      resultCopy: 'The result returns to the conversation with an explanation of what changed, leaving the decision to apply it with you.',
      diff: '+ if (results.length === 0) {\n+   return <EmptyState message="No results yet." />;\n+ }',
      stages: [
        'Describe what you need and let Orchestrix organize the next steps around your conversation.',
        'Relevant context brings together SearchResults.tsx and the search empty-state behavior to keep the work focused.',
        'Planning, development, and review connect to prepare a clear, welcoming empty state.',
        'Compare the suggested message and snippet, then decide whether to approve, request a change, or continue the conversation.'
      ]
    },
    create: {
      name: 'Create',
      request: 'Create a page to introduce Aurora, with a project description and a contact form.',
      context: 'Project Aurora · visual identity · page requirements, guided by the project preferences that give your conversation continuity.',
      plan: 'Define the structure → prepare the components → check clarity and accessibility.',
      resultTitle: 'The first version takes shape',
      resultCopy: 'Review the proposed structure and refine its direction before moving forward with the project.',
      diff: '+ <main>\n+   <ProjectIntro title="Aurora" />\n+   <ProjectDetails />\n+   <ContactForm />\n+ </main>',
      stages: [
        'Start with the idea, without having to assign tasks or configure every agent.',
        'Aurora’s identity and the page requirements guide the proposal.',
        'The roles coordinate to organize the structure, prepare the components, and review the experience.',
        'Receive a first proposal and adjust content, appearance, and scope in the same conversation.'
      ]
    },
    understand: {
      name: 'Understand',
      request: 'Explain how authentication works in this project before suggesting changes.',
      context: 'Sign-in flow · session · access control, following the question you asked in chat.',
      plan: 'Locate the flow → connect the steps → explain the points to watch.',
      resultTitle: 'Clarity to choose the next step',
      resultCopy: 'An explanation is a result too, bringing the walkthrough together so you can understand the project before considering changes.',
      diff: 'login.ts → session.ts → access.ts\n\n1. Sign-in validates identity.\n2. The session maintains access.\n3. Routes check permissions.',
      stages: [
        'Ask for an explanation and set the boundary: understand the project before proposing changes.',
        'Relevant context gathers the parts of the authentication flow that help answer the question.',
        'The roles organize the walkthrough and check that the explanation connects the steps clearly.',
        'Receive a summary of the flow and choose the next step, with understanding as the goal rather than making changes.'
      ]
    }
  };

  const stageTitles = [
    'It all starts with your goal',
    'Context gives the work direction',
    'Agents coordinate around one shared goal',
    'The final say stays with you'
  ];

  const body = document.body;
  const demo = document.querySelector('#product-demo');
  const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)');
  const status = document.querySelector('#demo-live');
  const playButton = document.querySelector('#demo-play');
  const canvas = document.querySelector('#ambient-field');
  const experience = document.querySelector('#experience-preview');
  const inspector = document.querySelector('#studio-inspector');
  let scenario = 'fix';
  let step = 0;
  let playing = false;
  let playTimer = 0;
  let demoVisible = true;
  let ambientVisible = true;
  let heroVisible = true;
  let pageActive = true;

  function write(selector, value) {
    const element = document.querySelector(selector);
    if (element) element.textContent = value;
  }

  function announce(message) {
    if (status) status.textContent = message;
  }

  function motionAllowed() {
    return !motionQuery.matches;
  }

  function stopPlayTimer() {
    window.clearTimeout(playTimer);
    playTimer = 0;
  }

  function syncPlayButton() {
    if (!playButton) return;
    playButton.setAttribute('aria-pressed', String(playing));
    playButton.disabled = !motionAllowed();
    const label = playButton.querySelector('.play-label');
    if (label) label.textContent = playing ? 'Pause workflow' : 'Play workflow';
    playButton.querySelector('svg use')?.setAttribute('href', playing ? '#i-pause' : '#i-play');
    if (motionQuery.matches) playButton.title = 'Reduced motion: use the manual steps.';
    else playButton.removeAttribute('title');
  }

  function setPlaying(value, feedback = false) {
    playing = Boolean(value) && motionAllowed();
    stopPlayTimer();
    syncPlayButton();
    if (feedback) announce(playing ? 'Playback started and will continue through to review.' : 'Playback paused so you can move through the steps manually.');
    schedulePlay();
  }

  function schedulePlay() {
    stopPlayTimer();
    if (!playing || !motionAllowed() || !demoVisible || document.hidden || !pageActive) return;
    playTimer = window.setTimeout(() => {
      playTimer = 0;
      if (step < 3) renderStep(step + 1);
      if (step === 3) {
        setPlaying(false);
        announce('The workflow is complete, with the review available for you to explore.');
      } else {
        schedulePlay();
      }
    }, 4300);
  }

  function renderStep(nextStep, feedback = false) {
    step = Math.max(0, Math.min(3, nextStep));
    if (demo) {
      demo.dataset.stage = String(step);
      demo.dataset.scenario = scenario;
    }
    const content = scenarios[scenario];
    write('#demo-request', content.request);
    write('#demo-context', content.context);
    write('#demo-plan', content.plan);
    write('#demo-result-title', content.resultTitle);
    write('#demo-result-copy', content.resultCopy);
    const diff = document.querySelector('#demo-diff');
    if (diff) {
      const code = diff.querySelector('code');
      (code || diff).textContent = content.diff;
    }
    write('#demo-stage-title', stageTitles[step]);
    write('#demo-stage-copy', content.stages[step]);

    document.querySelectorAll('[data-scenario]').forEach((button) => {
      if (button === demo) return;
      if (button.matches('button')) button.setAttribute('aria-pressed', String(button.dataset.scenario === scenario));
    });
    document.querySelectorAll('[data-demo-step]').forEach((button) => {
      const index = Number(button.dataset.demoStep);
      if (index === step) button.setAttribute('aria-current', 'step');
      else button.removeAttribute('aria-current');
      button.classList.toggle('is-complete', index < step);
    });
    document.querySelectorAll('[data-agent]').forEach((agent) => {
      const activeAt = { planner: 1, coder: 2, reviewer: 3 }[agent.dataset.agent];
      agent.classList.toggle('is-active', activeAt === step);
      agent.classList.toggle('is-complete', activeAt < step);
    });
    const previous = document.querySelector('#demo-prev');
    const next = document.querySelector('#demo-next');
    if (previous) previous.disabled = step === 0;
    if (next) next.disabled = step === 3;
    if (feedback) announce(`${content.name}: step ${step + 1} of 4. ${stageTitles[step]}`);
  }

  document.querySelectorAll('button[data-scenario]').forEach((button) => {
    button.addEventListener('click', () => {
      if (!Object.hasOwn(scenarios, button.dataset.scenario)) return;
      scenario = button.dataset.scenario;
      setPlaying(false);
      renderStep(0, true);
    });
  });

  document.querySelectorAll('[data-demo-step]').forEach((button) => {
    button.addEventListener('click', () => {
      setPlaying(false);
      renderStep(Number(button.dataset.demoStep), true);
    });
  });

  document.querySelector('#demo-next')?.addEventListener('click', () => {
    setPlaying(false);
    renderStep(step + 1, true);
  });
  document.querySelector('#demo-prev')?.addEventListener('click', () => {
    setPlaying(false);
    renderStep(step - 1, true);
  });
  document.querySelector('#demo-restart')?.addEventListener('click', () => {
    setPlaying(false);
    renderStep(0, true);
  });
  playButton?.addEventListener('click', () => {
    if (!playing && step === 3) renderStep(0);
    setPlaying(!playing, true);
  });

  function chooseExperience(mode, feedback = false) {
    if (mode !== 'chat' && mode !== 'studio') return;
    if (experience) experience.dataset.mode = mode;
    if (inspector) inspector.hidden = mode !== 'studio';
    document.querySelectorAll('button[data-experience]').forEach((button) => {
      button.setAttribute('aria-pressed', String(button.dataset.experience === mode));
    });
    write('#experience-description', mode === 'studio'
      ? 'Keep the same conversation in view while exploring context, orchestration decisions, and review whenever you want more control.'
      : 'Your request, the conversation, and the next step, with just what you need to move forward.');
    if (feedback) announce(mode === 'studio' ? 'Studio selected, showing the details for the same conversation.' : 'Chat selected, bringing the conversation to the foreground.');
  }

  document.querySelectorAll('button[data-experience]').forEach((button) => {
    button.addEventListener('click', () => chooseExperience(button.dataset.experience, true));
  });

  const context = canvas?.getContext('2d');
  const hero = document.querySelector('#hero');
  const network = document.querySelector('#hero-network') || document.querySelector('.constellation-lines');
  const emblem = document.querySelector('.hero-emblem');
  const orbitNodes = Array.from(document.querySelectorAll('[data-orbit-node]'));
  const precisePointer = window.matchMedia('(hover: hover) and (pointer: fine)');
  const pointer = { x: 0, y: 0, targetX: 0, targetY: 0, amount: 0, active: false, initialized: false, lastMove: 0 };
  let particles = [];
  let canvasWidth = 0;
  let canvasHeight = 0;
  let animationFrame = 0;
  let lastFrame = 0;
  let nextFieldFrame = 0;
  let fieldCadence = 1000 / 30;
  let lastOrbitFrame = 0;
  let elapsed = 0;
  let scrollImpulse = 0;
  let scrollDirection = 1;
  let previousScroll = window.scrollY;
  let previousScrollTime = performance.now();

  function seededRandom(seed) {
    let value = seed;
    return () => {
      value = (value * 1664525 + 1013904223) >>> 0;
      return value / 4294967296;
    };
  }

  function drawField() {
    if (!context || !canvasWidth || !canvasHeight) return;
    context.clearRect(0, 0, canvasWidth, canvasHeight);
    if (pointer.amount > 0.01) {
      const glow = context.createRadialGradient(pointer.x, pointer.y, 0, pointer.x, pointer.y, 260);
      glow.addColorStop(0, `rgba(99, 102, 241, ${0.085 * pointer.amount})`);
      glow.addColorStop(1, 'rgba(99, 102, 241, 0)');
      context.fillStyle = glow;
      context.fillRect(pointer.x - 260, pointer.y - 260, 520, 520);
    }
    particles.forEach((particle) => {
      const distanceX = particle.x - pointer.x;
      const distanceY = particle.y - pointer.y;
      const distance = Math.hypot(distanceX, distanceY);
      const proximity = Math.max(0, 1 - distance / 220) * pointer.amount;
      const weave = motionAllowed() ? Math.sin(elapsed * 0.0003 + particle.phase) * 2.5 * particle.depth : 0;
      const x = particle.x + particle.offsetX + weave;
      const y = particle.y + particle.offsetY;
      const pulse = motionAllowed() ? Math.sin(elapsed * 0.00035 + particle.phase) * 0.07 : 0;
      context.globalAlpha = Math.min(0.95, particle.opacity + pulse + proximity * 0.35);
      context.fillStyle = particle.color;
      context.beginPath();
      context.arc(x, y, particle.radius + proximity * 0.38, 0, Math.PI * 2);
      context.fill();
    });
    context.globalAlpha = 1;
  }

  // Rectangles are transformed through the SVG's screen matrix rather than
  // assuming it fills its CSS box: preserveAspectRatio can add letterboxing.
  function alignOrbit() {
    if (!network || !emblem || !orbitNodes.length || !network.getScreenCTM) return;
    const matrix = network.getScreenCTM();
    if (!matrix) return;
    const inverse = matrix.inverse();
    const position = (x, y) => new DOMPoint(x, y).matrixTransform(inverse);
    const centerRect = emblem.getBoundingClientRect();
    if (!centerRect.width || !centerRect.height) return;
    const centerX = centerRect.left + centerRect.width / 2;
    const centerY = centerRect.top + centerRect.height / 2;
    const radiusX = centerRect.width / 2;
    const radiusY = centerRect.height / 2;
    orbitNodes.forEach((node) => {
      const key = node.dataset.orbitNode;
      const path = network.querySelector(`[data-orbit-connector="${key}"]`);
      const dot = network.querySelector(`[data-orbit-dot="${key}"]`);
      if (!path || !dot) return;
      const rect = node.getBoundingClientRect();
      const isLeft = rect.left + rect.width / 2 < centerX;
      const endX = isLeft ? rect.right : rect.left;
      const endY = rect.top + rect.height / 2;
      const angle = Math.atan2((endY - centerY) / radiusY, (endX - centerX) / radiusX);
      const start = position(centerX + Math.cos(angle) * radiusX, centerY + Math.sin(angle) * radiusY);
      const end = position(endX, endY);
      const direction = isLeft ? -1 : 1;
      const reach = Math.max(16, Math.abs(end.x - start.x) * 0.48);
      path.setAttribute('d', `M${start.x.toFixed(2)} ${start.y.toFixed(2)} C${(start.x + direction * reach).toFixed(2)} ${start.y.toFixed(2)} ${(end.x - direction * reach).toFixed(2)} ${end.y.toFixed(2)} ${end.x.toFixed(2)} ${end.y.toFixed(2)}`);
      dot.setAttribute('cx', end.x.toFixed(2));
      dot.setAttribute('cy', end.y.toFixed(2));
    });
  }

  function drawOrbit() {
    if (!heroVisible || !orbitNodes.length) return;
    orbitNodes.forEach((node, index) => {
      const offset = motionAllowed() ? Math.sin(elapsed * 0.00036 + index * 1.4) * 3 : 0;
      node.style.transform = `translateY(${offset.toFixed(2)}px)`;
    });
    alignOrbit();
  }

  function canAnimate() {
    const fieldReady = context && canvasWidth && canvasHeight && ambientVisible;
    return motionAllowed() && (fieldReady || (network && heroVisible)) && !document.hidden && pageActive;
  }

  function stopField() {
    window.cancelAnimationFrame(animationFrame);
    animationFrame = 0;
    lastFrame = 0;
    nextFieldFrame = 0;
    lastOrbitFrame = 0;
  }

  function advanceParticles(delta) {
    const seconds = delta / 1000;
    const spring = 18;
    const decay = Math.exp(-spring * seconds);
    particles.forEach((particle) => {
      particle.x += particle.drift * seconds * (1 + scrollImpulse * 0.6);
      particle.y -= (particle.speed * (1 + scrollImpulse * 1.3) + scrollDirection * scrollImpulse * 2) * seconds;
      if (particle.y < -24) particle.y = canvasHeight + 24;
      if (particle.y > canvasHeight + 24) particle.y = -24;
      if (particle.x < -24) particle.x = canvasWidth + 24;
      if (particle.x > canvasWidth + 24) particle.x = -24;
      const distanceX = particle.x - pointer.x;
      const distanceY = particle.y - pointer.y;
      const distance = Math.hypot(distanceX, distanceY);
      const proximity = Math.max(0, 1 - distance / 220);
      const repel = proximity * proximity * 34 * pointer.amount;
      const safeDistance = Math.max(distance, 1);
      const targetX = distanceX / safeDistance * repel + (pointer.x / canvasWidth - 0.5) * particle.depth * 11 * pointer.amount;
      const targetY = distanceY / safeDistance * repel + (pointer.y / canvasHeight - 0.5) * particle.depth * 8 * pointer.amount;
      // The exact critically damped solution retains momentum without jitter
      // or unstable Euler steps when frame timing changes.
      const shiftX = particle.offsetX - targetX;
      const shiftY = particle.offsetY - targetY;
      const momentumX = (particle.velocityX + spring * shiftX) * seconds;
      const momentumY = (particle.velocityY + spring * shiftY) * seconds;
      particle.offsetX = targetX + (shiftX + momentumX) * decay;
      particle.offsetY = targetY + (shiftY + momentumY) * decay;
      particle.velocityX = (particle.velocityX - spring * momentumX) * decay;
      particle.velocityY = (particle.velocityY - spring * momentumY) * decay;
    });
  }

  function frame(time) {
    animationFrame = 0;
    if (!canAnimate()) return;
    // Stars respond at 60 fps during interaction; connector layout stays at
    // 30 fps. The quiet ambient field also returns to 30 fps.
    const pointerChanging = time - pointer.lastMove < 260 || Math.abs(pointer.amount - (pointer.active ? 1 : 0)) > 0.01;
    const fieldInterval = 1000 / (pointerChanging || scrollImpulse > 0.03 ? 60 : 30);
    if (fieldInterval !== fieldCadence) {
      fieldCadence = fieldInterval;
      nextFieldFrame = lastFrame ? lastFrame + fieldInterval : 0;
    }
    if (!nextFieldFrame || time >= nextFieldFrame) {
      const delta = lastFrame ? Math.min(time - lastFrame, 100) : 0;
      lastFrame = time;
      nextFieldFrame = nextFieldFrame ? nextFieldFrame + fieldInterval : time + fieldInterval;
      if (nextFieldFrame <= time) nextFieldFrame = time + fieldInterval;
      elapsed += delta;
      scrollImpulse *= Math.exp(-delta / 540);
      const smoothing = 1 - Math.exp(-delta / 70);
      pointer.x += (pointer.targetX - pointer.x) * smoothing;
      pointer.y += (pointer.targetY - pointer.y) * smoothing;
      const fade = 1 - Math.exp(-delta / (pointer.active ? 180 : 400));
      pointer.amount += ((pointer.active ? 1 : 0) - pointer.amount) * fade;
      if (ambientVisible && context && canvasWidth && canvasHeight) {
        advanceParticles(delta);
        drawField();
      }
    }
    if (!lastOrbitFrame || time - lastOrbitFrame >= 1000 / 30 - 0.5) {
      lastOrbitFrame = time;
      drawOrbit();
    }
    animationFrame = window.requestAnimationFrame(frame);
  }

  function syncField() {
    if (canAnimate()) {
      if (!animationFrame) animationFrame = window.requestAnimationFrame(frame);
    } else {
      stopField();
      if (!motionAllowed()) {
        pointer.amount = 0;
        pointer.active = false;
        scrollImpulse = 0;
        particles.forEach((particle) => {
          particle.offsetX = 0;
          particle.offsetY = 0;
          particle.velocityX = 0;
          particle.velocityY = 0;
        });
        drawOrbit();
      }
      drawField();
    }
  }

  function fitField() {
    if (!canvas || !context) return;
    const rect = canvas.getBoundingClientRect();
    const previousWidth = canvasWidth;
    const previousHeight = canvasHeight;
    const previousParticles = particles;
    canvasWidth = Math.round(rect.width);
    canvasHeight = Math.round(rect.height);
    const ratio = Math.min(window.devicePixelRatio || 1, 1.5);
    canvas.width = Math.round(canvasWidth * ratio);
    canvas.height = Math.round(canvasHeight * ratio);
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
    const random = seededRandom(1447);
    const count = Math.min(72, Math.max(22, Math.floor(canvasWidth * canvasHeight / 21000)));
    particles = Array.from({ length: count }, (_, index) => {
      const particle = {
        x: random() * canvasWidth,
        y: random() * canvasHeight,
        radius: 0.7 + random() * 0.85,
        opacity: 0.25 + random() * 0.35,
        phase: random() * Math.PI * 2,
        speed: 2.3 + random() * 2.7,
        depth: 0.35 + random() * 0.65,
        drift: (random() - 0.5) * 1.2,
        color: random() > 0.65 ? '#9EA5FF' : '#839DCC',
        offsetX: 0,
        offsetY: 0,
        velocityX: 0,
        velocityY: 0
      };
      const previous = previousParticles[index];
      if (previous && previousWidth && previousHeight) {
        previous.x *= canvasWidth / previousWidth;
        previous.y *= canvasHeight / previousHeight;
        return previous;
      }
      return particle;
    });
    drawField();
    syncField();
  }

  window.addEventListener('scroll', () => {
    const time = performance.now();
    const distance = window.scrollY - previousScroll;
    if (motionAllowed() && distance) {
      const delta = Math.max(16, Math.min(150, time - previousScrollTime));
      scrollImpulse = Math.min(2.5, Math.max(scrollImpulse, Math.abs(distance) / delta * 0.65));
      scrollDirection = Math.sign(distance);
    }
    previousScroll = window.scrollY;
    previousScrollTime = time;
  }, { passive: true });

  window.addEventListener('pointermove', (event) => {
    if (!motionAllowed() || !precisePointer.matches || event.pointerType === 'touch') return;
    if (!pointer.initialized) {
      pointer.x = event.clientX;
      pointer.y = event.clientY;
      pointer.initialized = true;
    }
    pointer.targetX = event.clientX;
    pointer.targetY = event.clientY;
    pointer.active = true;
    pointer.lastMove = performance.now();
  }, { passive: true });
  window.addEventListener('pointerout', (event) => {
    if (!event.relatedTarget) pointer.active = false;
  }, { passive: true });
  window.addEventListener('blur', () => { pointer.active = false; });
  precisePointer.addEventListener('change', (event) => {
    if (!event.matches) pointer.active = false;
  });

  function syncMotion() {
    body.dataset.motion = motionQuery.matches ? 'reduced' : 'running';
    body.classList.toggle('is-background-paused', document.hidden || !pageActive);
    if (!motionAllowed()) setPlaying(false);
    else syncPlayButton();
    syncField();
    schedulePlay();
  }

  if (motionQuery.addEventListener) motionQuery.addEventListener('change', syncMotion);
  else motionQuery.addListener(syncMotion);

  document.addEventListener('visibilitychange', syncMotion);
  window.addEventListener('pagehide', () => {
    pageActive = false;
    stopField();
    stopPlayTimer();
    body.classList.add('is-background-paused');
  });
  window.addEventListener('pageshow', () => {
    pageActive = true;
    previousScroll = window.scrollY;
    previousScrollTime = performance.now();
    syncMotion();
    alignOrbit();
  });

  if ('IntersectionObserver' in window) {
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (entry.target === demo) {
          demoVisible = entry.isIntersecting;
          demo.classList.toggle('is-offscreen', !demoVisible);
          schedulePlay();
        }
        if (entry.target === canvas) ambientVisible = entry.isIntersecting;
        if (entry.target === hero) {
          heroVisible = entry.isIntersecting;
          hero.classList.toggle('is-offscreen', !heroVisible);
          if (heroVisible) alignOrbit();
        }
      });
      syncField();
    });
    if (demo) observer.observe(demo);
    if (canvas) observer.observe(canvas);
    if (hero) observer.observe(hero);
  }

  if (canvas && context) {
    if ('ResizeObserver' in window) new ResizeObserver(fitField).observe(canvas);
    else window.addEventListener('resize', fitField);
    fitField();
  }
  if (network && emblem) {
    if ('ResizeObserver' in window) {
      const orbitObserver = new ResizeObserver(alignOrbit);
      orbitObserver.observe(network);
      orbitObserver.observe(emblem);
      orbitNodes.forEach((node) => orbitObserver.observe(node));
    } else window.addEventListener('resize', alignOrbit);
    document.fonts?.ready.then(alignOrbit);
    alignOrbit();
  }

  renderStep(0);
  chooseExperience('chat');
  syncMotion();
  document.querySelectorAll('[data-enhanced]').forEach((element) => {
    element.hidden = false;
  });
  body.classList.add('is-enhanced');
})();
