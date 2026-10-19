(() => {
  'use strict';
  document.documentElement.classList.add('js');
  document.getElementById('year').textContent = String(new Date().getFullYear());
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)');
  let paused = reduced.matches;
  const toggle = document.getElementById('motion-toggle');
  function updateMotion() {
    document.body.classList.toggle('motion-paused', paused);
    toggle.setAttribute('aria-pressed', String(paused));
    toggle.setAttribute('aria-label', paused ? 'Ativar animações' : 'Pausar animações');
    toggle.querySelector('.motion-icon').textContent = paused ? '▷' : 'Ⅱ';
    toggle.querySelector('.motion-label').textContent = paused ? 'ATIVAR EFEITOS' : 'PAUSAR EFEITOS';
  }
  toggle.addEventListener('click', () => { paused = !paused; updateMotion(); });
  reduced.addEventListener('change', () => { paused = reduced.matches; updateMotion(); });
  updateMotion();

  const reveal = new IntersectionObserver(entries => {
    for (const entry of entries) if (entry.isIntersecting) {
      entry.target.classList.add('visible');
      reveal.unobserve(entry.target);
    }
  }, { threshold: 0.08 });
  document.querySelectorAll('.reveal').forEach(element => reveal.observe(element));

  const stages = [
    { label: 'IDENTIFICAR', title: 'O contexto vem primeiro.', text: 'Rede, contrato, versão e estado de referência. O motor registra a identidade da entrada para que cada leitura tenha uma origem verificável.', code: 'INPUT / IDENTITY', tags: ['IDENTIDADE', 'ESTADO DE REFERÊNCIA', 'PROVENIÊNCIA'] },
    { label: 'RECONSTRUIR', title: 'Revelar as conexões do código.', text: 'Funções, caminhos de execução e relações de estado compõem o mapa do contrato. A reconstrução reúne source e bytecode, mantendo explícito o que ainda precisa ser compreendido.', code: 'SOURCE / BYTECODE', tags: ['FUNÇÕES', 'FLUXOS DE EXECUÇÃO', 'RELAÇÕES DE ESTADO'] },
    { label: 'APROFUNDAR', title: 'Dar atenção aos detalhes.', text: 'Pontos que merecem revisão seguem para uma análise dedicada. Relações econômicas e cenários locais ajudam a comparar o comportamento esperado com as evidências observadas.', code: 'CONTEXT / LOCAL SCENARIOS', tags: ['ANÁLISE ECONÔMICA', 'CENÁRIOS LOCAIS', 'REVISÃO DEDICADA'] },
    { label: 'REAVALIAR', title: 'Voltar ao todo, com mais contexto.', text: 'As novas evidências retornam ao mapa inicial. O motor reavalia as prioridades e preserva as condições e os limites de cada leitura para apoiar a revisão dos resultados.', code: 'EVIDENCE / REASSESSMENT', tags: ['EVIDÊNCIAS', 'REAVALIAÇÃO', 'RASTREABILIDADE'] }
  ];
  const tabs = [...document.querySelectorAll('[data-stage]')];
  function selectStage(index, focus = false) {
    const stage = stages[index];
    tabs.forEach((tab, i) => { tab.setAttribute('aria-selected', String(i === index)); tab.tabIndex = i === index ? 0 : -1; });
    document.getElementById('stage-panel').setAttribute('aria-labelledby', `tab-${index}`);
    document.getElementById('stage-label').textContent = `0${index + 1} / ${stage.label}`;
    document.getElementById('stage-glyph').textContent = `0${index + 1}`;
    document.getElementById('stage-title').textContent = stage.title;
    document.getElementById('stage-description').textContent = stage.text;
    document.getElementById('stage-code').textContent = stage.code;
    document.getElementById('stage-tags').replaceChildren(...stage.tags.map(text => { const span = document.createElement('span'); span.textContent = text; return span; }));
    if (focus) tabs[index].focus();
  }
  tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => selectStage(index));
    tab.addEventListener('keydown', event => {
      const keys = { ArrowRight: (index + 1) % tabs.length, ArrowLeft: (index + tabs.length - 1) % tabs.length, Home: 0, End: tabs.length - 1 };
      if (Object.hasOwn(keys, event.key)) { event.preventDefault(); selectStage(keys[event.key], true); }
    });
  });

  const hero = document.querySelector('.hero');
  const heroImage = document.querySelector('.hero-image');
  const pointerFine = window.matchMedia('(pointer: fine)');
  hero.addEventListener('pointermove', event => {
    if (paused || !pointerFine.matches) return;
    const rect = hero.getBoundingClientRect();
    heroImage.style.setProperty('--px', `${(event.clientX / rect.width - 0.5) * 12}px`);
    heroImage.style.setProperty('--py', `${((event.clientY - rect.top) / rect.height - 0.5) * 8}px`);
  });
  hero.addEventListener('pointerleave', () => { heroImage.style.setProperty('--px', '0px'); heroImage.style.setProperty('--py', '0px'); });

  const canvas = document.getElementById('atmosphere');
  const ctx = canvas.getContext('2d', { alpha: true });
  if (!ctx) return;
  let width = 1, height = 1, visible = true, last = 0, phase = 0;
  const fog = document.createElement('canvas');
  fog.width = fog.height = 384;
  const fogCtx = fog.getContext('2d');
  // Procedural translucent cloud texture; no external animation library.
  for (let i = 0; i < 38; i++) {
    const angle = i * 2.39996;
    const distance = Math.sqrt(i / 38) * 100;
    const x = 192 + Math.cos(angle) * distance, y = 192 + Math.sin(angle) * distance * .65;
    const radius = 50 + (i % 7) * 6;
    const gradient = fogCtx.createRadialGradient(x, y, 0, x, y, radius);
    gradient.addColorStop(0, 'rgba(171,212,241,.055)');
    gradient.addColorStop(.5, 'rgba(119,183,230,.028)');
    gradient.addColorStop(1, 'rgba(95,160,220,0)');
    fogCtx.fillStyle = gradient; fogCtx.fillRect(0, 0, 384, 384);
  }
  const motes = Array.from({ length: 44 }, (_, i) => ({ x: ((i * 73.31) % 100) / 100, y: ((i * 31.77) % 100) / 100, size: 0.7 + i % 3 * .5, speed: .3 + i % 7 * .12 }));
  function resize() {
    width = hero.clientWidth; height = hero.clientHeight;
    const scale = Math.min(window.devicePixelRatio || 1, 1.5);
    canvas.width = Math.round(width * scale); canvas.height = Math.round(height * scale);
    ctx.setTransform(scale, 0, 0, scale, 0, 0);
  }
  new ResizeObserver(resize).observe(hero); resize();
  new IntersectionObserver(entries => { visible = entries[0].isIntersecting; }).observe(hero);
  function draw(now) {
    requestAnimationFrame(draw);
    if (!visible || document.hidden || paused) { last = now; return; }
    if (now - last < 33) return;
    phase += Math.min((now - last) / 1000, .08); last = now;
    ctx.clearRect(0, 0, width, height);
    for (let i = 0; i < 8; i++) {
      const size = Math.max(width * .48, 310) + i % 3 * 70;
      const x = ((i * .185 + phase * (i % 2 ? .003 : -.002) + 10) % 1.5 - .25) * width;
      const y = height * (.65 + Math.sin(phase * .1 + i * 2) * .13);
      ctx.globalAlpha = .35 + (i % 3) * .12;
      ctx.drawImage(fog, x - size / 2, y - size / 2, size, size * .72);
    }
    for (const mote of motes) {
      const x = (mote.x * width + Math.sin(phase * .12 + mote.y * 6) * 18 + width) % width;
      const y = ((mote.y * height - phase * mote.speed * 5) % height + height) % height;
      ctx.globalAlpha = .2 + Math.sin(phase + mote.x * 10) ** 2 * .45;
      ctx.fillStyle = y < height * .4 ? '#2473b4' : '#71d6ff';
      ctx.fillRect(x, y, mote.size, mote.size);
    }
    ctx.globalAlpha = 1;
  }
  requestAnimationFrame(draw);
})();


(() => {
  const form = document.getElementById('smart-form');
  const events = document.getElementById('smart-events');
  const state = document.getElementById('smart-state');
  const report = document.getElementById('smart-report');
  const reportText = document.getElementById('smart-report-text');
  const allowed = '0x8a1d45e102e886510e891d2ec656a708991e2d76';
  let timer = null;
  let password = '';

  function headers() {
    return {
      'Content-Type': 'application/json',
      'Authorization': 'Basic ' + btoa('owner:' + password)
    };
  }

  function line(time, message) {
    const item = document.createElement('li');
    const stamp = document.createElement('time');
    stamp.textContent = time || '—';
    const content = document.createElement('span');
    content.textContent = message;
    item.append(stamp, content);
    return item;
  }

  function draw(data) {
    state.textContent = {
      idle: 'AGUARDANDO', running: 'EM CURSO',
      complete: 'CONCLUÍDO', failed: 'INCOMPLETO'
    }[data.state] || '—';
    const rows = (data.events || []).map(item =>
      line((item.time || '').slice(11, 19), item.name + ' — ' + item.detail));
    events.replaceChildren(...(rows.length ? rows : [line('—', 'Aguardando nova leitura.')]));
    if (data.report) {
      reportText.textContent = data.report;
      report.hidden = false;
    } else {
      report.hidden = true;
    }
    clearTimeout(timer);
    if (data.state === 'running') timer = setTimeout(refresh, 3000);
  }

  async function refresh() {
    if (!password) return;
    try {
      const response = await fetch('/api/smart/latest', {
        headers: headers(), credentials: 'same-origin', cache: 'no-store'
      });
      if (!response.ok) throw new Error(response.status === 401 ?
        'Chave de acesso incorreta.' : 'Não foi possível ler o andamento.');
      draw(await response.json());
    } catch (error) {
      events.replaceChildren(line('!', error.message));
      state.textContent = 'SEM CONEXÃO';
    }
  }

  form.addEventListener('submit', async event => {
    event.preventDefault();
    const address = form.elements.address.value.trim().toLowerCase();
    password = form.elements.password.value;
    if (address !== allowed) {
      events.replaceChildren(line('!', 'Esta etapa aceita apenas o endereço wCOP exibido acima.'));
      return;
    }
    state.textContent = 'INICIANDO';
    events.replaceChildren(line('…', 'Enviando endereço ao VPS.'));
    try {
      const response = await fetch('/api/smart', {
        method: 'POST', headers: headers(), credentials: 'same-origin',
        body: JSON.stringify({ address })
      });
      if (response.status === 409) {
        await refresh();
        return;
      }
      if (!response.ok) throw new Error(response.status === 401 ?
        'Chave de acesso incorreta.' : 'Falha ao iniciar.');
      draw(await response.json());
      await refresh();
    } catch (error) {
      events.replaceChildren(line('!', error.message));
      state.textContent = 'NÃO INICIADO';
    }
  });

  document.getElementById('smart-copy').addEventListener('click', async () => {
    await navigator.clipboard.writeText(reportText.textContent);
  });
})();
