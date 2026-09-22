// 簡單線條圖示由此集中管理，不依賴網路字型或平台符號字形。
const icons = {
  pen: '<path d="m15 5 4 4M4 20l4-1L20 7a2.8 2.8 0 0 0-4-4L4 15Z"/>',
  pin: '<path d="m16 3 5 5-4 1-4 4 1 4-2 2-7-7 2-2 4 1 4-4ZM3 21l5-5"/>',
  monitor: '<rect x="3" y="3" width="18" height="13" rx="2"/><path d="M8 21h8M12 16v5"/>',
  paperclip:
    '<path d="m8 13 6-6a3 3 0 0 1 4 4l-8 8a5 5 0 0 1-7-7l9-9a2 2 0 0 1 3 3l-9 9"/>',
  panel: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="M9 4v16"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  bell: '<path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/>',
  mail: '<rect x="3" y="5" width="18" height="14" rx="3"/><path d="m4 7 8 6 8-6"/>',
  settings:
    '<path d="m9 3-.8 2.5-2.6.5-1.4 2.4 1 2.5-1 2.5 1.4 2.4 2.6.5L9 19h3l.8-2.7 2.6-.5 1.4-2.4-1-2.5 1-2.5-1.4-2.4-2.6-.5L12 3Z" transform="translate(1 1)"/><circle cx="11.5" cy="12" r="3"/>',
  lock: '<rect x="5" y="10" width="14" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>',
  trash: '<path d="M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v6M14 10v6"/>',
  "arrow-down": '<path d="M12 4v16m-6-6 6 6 6-6"/>',
  "arrow-up": '<path d="M12 20V4m-6 6 6-6 6 6"/>',
  "chevron-down": '<path d="m6 9 6 6 6-6"/>',
  copy: '<rect x="8" y="8" width="12" height="13" rx="2"/><path d="M16 8V3H3v13h5"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  x: '<path d="m6 6 12 12M6 18 18 6"/>',
  refresh:
    '<path d="M20 7v5h-5M4 17v-5h5M5 8a8 8 0 0 1 13-3l2 3M4 16l2 3a8 8 0 0 0 13-3"/>',
  download: '<path d="M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4"/>',
  shield:
    '<path d="m12 3 8 3v6c0 5-8 9-8 9s-8-4-8-9V6Z"/><path d="m8 11 3 3 5-6"/>',
  languages:
    '<path d="M3 5h12M9 3v2M5 5c0 5 4 8 8 10M13 5c0 5-4 9-10 12M13 21l4-10 4 10M15 17h4"/>',
  list: '<path d="M9 6h12M9 12h12M9 18h12M3 6h1M3 12h1M3 18h1"/>',
  sparkles:
    '<path d="m12 3 2.5 6.5L21 12l-6.5 2.5L12 21l-2.5-6.5L3 12l6.5-2.5ZM20 2v4M18 4h4"/>',
  chat: '<path d="M5 4h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H9l-5 3v-3H3V6a2 2 0 0 1 2-2Z"/><path d="M7 9h10M7 13h6"/>',
  user: '<circle cx="12" cy="8" r="4"/><path d="M4 21v-2a8 8 0 0 1 16 0v2"/>',
};
function icon(name) {
  return `<svg viewBox="0 0 24 24" aria-hidden="true">${icons[name] || icons.chat}</svg>`;
}
function fillIcons(root = document) {
  root.querySelectorAll("[data-icon]").forEach((el) => {
    el.innerHTML = icon(el.dataset.icon);
  });
}
fillIcons();
