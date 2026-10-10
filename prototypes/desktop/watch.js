(() => {
  'use strict';
  const video = document.querySelector('#product-video');
  const placeholder = document.querySelector('#video-placeholder');
  const poster = document.querySelector('.video-poster');
  const transcript = document.querySelector('#video-transcript');
  const status = document.querySelector('#video-status');
  const mediaURL = value => {
    if (typeof value !== 'string' || !value.trim()) return null;
    try {
      const url = new URL(value, document.baseURI);
      return ['http:', 'https:'].includes(url.protocol) ? url.href : null;
    } catch {return null;}
  };
  if (!video || !placeholder) return;
  fetch('video-config.json').then(response => {
    if (!response.ok) throw new Error('Video configuration unavailable');
    return response.json();
  }).then(config => {
    const source = mediaURL(config.src);
    if (!source) return;
    const image = mediaURL(config.poster);
    if (image) video.poster = image;
    const captions = mediaURL(config.captions);
    if (captions) {
      const track = document.createElement('track');
      Object.assign(track, {kind:'captions', srclang:'en', label:'English', src:captions, default:true});
      video.append(track);
    }
    if (typeof config.transcript === 'string' && config.transcript.trim() && transcript) {
      transcript.querySelector('p').textContent = config.transcript;
      transcript.hidden = false;
    }
    video.src = source;
    video.hidden = false;
    placeholder.hidden = true;
    if (poster) poster.hidden = true;
    video.addEventListener('error', () => {
      video.hidden = true;
      placeholder.hidden = false;
      if (poster) poster.hidden = false;
      if (status) status.textContent = 'The video could not be loaded, so please try again later.';
    }, {once:true});
  }).catch(() => {
    // The video page remains readable before media is configured or offline.
  });
})();
