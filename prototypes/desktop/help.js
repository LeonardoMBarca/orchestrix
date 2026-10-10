(() => {
  'use strict';
  const input = document.querySelector('#help-search');
  const form = document.querySelector('#help-search-form');
  const clear = document.querySelector('#clear-search');
  const status = document.querySelector('#search-status');
  const empty = document.querySelector('#empty-search');
  const guides = [...document.querySelectorAll('[data-guide]')];
  const normalize = value => value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
  const searchable = new Map(guides.map(guide => [guide, normalize(guide.textContent)]));

  function filterGuides() {
    const query = input.value.trim();
    const terms = normalize(query).split(/\s+/).filter(Boolean);
    let count = 0;
    for (const guide of guides) {
      const matches = terms.every(term => searchable.get(guide).includes(term));
      guide.hidden = !matches;
      if (matches) count += 1;
    }
    clear.hidden = !query;
    empty.hidden = count !== 0;
    status.textContent = query
      ? `${count} ${count === 1 ? 'guide matches' : 'guides match'} your search`
      : `${guides.length} guides to help you get started`;
  }
  input.addEventListener('input', filterGuides);
  form.addEventListener('submit', event => event.preventDefault());
  clear.addEventListener('click', () => {
    input.value = '';
    filterGuides();
    input.focus();
  });
  document.querySelectorAll('.help-nav a[href^="#"]').forEach(link => {
    link.addEventListener('click', () => {
      input.value = '';
      filterGuides();
      document.querySelectorAll('.help-nav a[aria-current]').forEach(item => item.removeAttribute('aria-current'));
      link.setAttribute('aria-current', 'location');
    });
  });

  // Publish a real source here when a guide is available. Empty slots stay hidden.
  // Paths relative to help.html work over HTTP and direct file:// opening.
  const videoGuides = {
    quickstart: {src: '', captions: '', transcript: ''},
    sessions: {src: '', captions: '', transcript: ''},
    review: {src: '', captions: '', transcript: ''},
  };
  const mediaURL = value => {
    if (!value) return null;
    try {
      const url = new URL(value, document.baseURI);
      return ['http:', 'https:', 'file:'].includes(url.protocol) ? url.href : null;
    } catch { return null; }
  };
  for (const [id, configuration] of Object.entries(videoGuides)) {
    const slot = document.querySelector(`[data-video-guide="${id}"]`);
    const src = mediaURL(configuration.src);
    if (!slot || !src) continue;
    const video = document.createElement('video');
    Object.assign(video, {src, controls: true, preload: 'none', playsInline: true});
    video.setAttribute('aria-label', `${document.querySelector(`#${id} h2`).textContent} video guide`);
    const captions = mediaURL(configuration.captions);
    if (captions) {
      const track = document.createElement('track');
      Object.assign(track, {kind: 'captions', src: captions, srclang: 'en', label: 'English', default: true});
      video.append(track);
    }
    slot.append(video);
    const transcript = mediaURL(configuration.transcript);
    if (transcript) {
      const link = document.createElement('a');
      Object.assign(link, {href: transcript, textContent: 'Read the transcript'});
      slot.append(link);
    }
    video.addEventListener('error', () => {
      video.hidden = true;
      const message = document.createElement('p');
      message.className = 'video-error';
      message.textContent = 'This video could not be loaded. The written guide is available above.';
      message.setAttribute('role', 'status');
      slot.append(message);
    }, {once: true});
    slot.hidden = false;
  }
})();
