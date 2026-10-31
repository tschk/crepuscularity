/* Progressive enhancement: without JavaScript the original navigation stays available. */
for (const source of document.querySelectorAll(
  'aside > nav[aria-label="Documentation"]',
)) {
  const switcher = document.createElement("div");
  switcher.className = "page-switcher";
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "page-switcher-toggle";
  toggle.setAttribute("aria-label", "Switch page");
  toggle.setAttribute("aria-expanded", "false");
  for (let i = 0; i < 3; i++) toggle.append(document.createElement("span"));
  const list = document.createElement("nav");
  list.className = "page-switcher-list";
  list.id = "page-switcher-links";
  list.setAttribute("aria-label", "Pages");
  list.hidden = true;
  toggle.setAttribute("aria-controls", list.id);
  const links = Array.from(source.querySelectorAll("a"), (link) => {
    const copy = link.cloneNode(true);
    copy.removeAttribute("class");
    copy.textContent = link.getAttribute("aria-label") || link.textContent;
    if (link.classList.contains("active"))
      copy.setAttribute("aria-current", "page");
    list.append(copy);
    return copy;
  });
  switcher.append(toggle, list);
  source.after(switcher);
  source.classList.add("switcher-source");
  source.parentElement.classList.add("switcher-header");
  const mobile = matchMedia("(max-width: 767px)");
  const search = source.querySelector(".doc-search");
  function moveSearch() {
    if (search) (mobile.matches ? list : source).prepend(search);
  }
  moveSearch();
  let active = null;
  let touching = false;
  let frame = 0;
  let touchY = 0;
  let enteredList = false;
  let closeOnTap = false;
  function close(restore = false) {
    touching = false;
    cancelAnimationFrame(frame);
    list.hidden = true;
    toggle.setAttribute("aria-expanded", "false");
    active?.classList.remove("gesture-active");
    active = null;
    if (restore) toggle.focus();
  }
  function open(keyboard = false) {
    switcher.style.setProperty(
      "--switcher-top",
      `${switcher.getBoundingClientRect().top}px`,
    );
    list.hidden = false;
    toggle.setAttribute("aria-expanded", "true");
    if (keyboard)
      (
        links.find((link) => link.hasAttribute("aria-current")) || links[0]
      )?.focus();
  }
  toggle.addEventListener("click", (event) => {
    if (list.hidden) open(event.detail === 0);
    else close();
  });
  toggle.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      open(true);
    }
  });
  switcher.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close(true);
    }
  });
  document.addEventListener("pointerdown", (event) => {
    if (!switcher.contains(event.target)) close();
  });
  document.addEventListener("focusin", (event) => {
    if (!switcher.contains(event.target)) close();
  });
  list.addEventListener("click", (event) => {
    if (event.target.closest("a")) close();
  });
  mobile.addEventListener("change", () => {
    close();
    moveSearch();
  });
  function selectNearest() {
    const rect = list.getBoundingClientRect();
    const searchRect = search?.getBoundingClientRect();
    if (searchRect && touchY < searchRect.bottom) {
      active?.classList.remove("gesture-active");
      active = null;
      if (touching) frame = requestAnimationFrame(selectNearest);
      return;
    }
    if (touchY >= rect.top) enteredList = true;
    if (!enteredList) {
      if (touching) frame = requestAnimationFrame(selectNearest);
      return;
    }
    if (touchY > rect.bottom - 36) list.scrollTop += 6;
    else if (touchY < rect.top + 36) list.scrollTop -= 6;
    const visible = links.filter((link) => {
      const box = link.getBoundingClientRect();
      return box.bottom > rect.top && box.top < rect.bottom;
    });
    active?.classList.remove("gesture-active");
    active = visible.reduce((best, link) => {
      const distance = (element) => {
        const box = element.getBoundingClientRect();
        return Math.abs((box.top + box.bottom) / 2 - touchY);
      };
      return !best || distance(link) < distance(best) ? link : best;
    }, null);
    active?.classList.add("gesture-active");
    if (touching) frame = requestAnimationFrame(selectNearest);
  }
  toggle.addEventListener(
    "touchstart",
    (event) => {
      if (!mobile.matches) return;
      event.preventDefault();
      closeOnTap = !list.hidden;
      enteredList = false;
      open();
      touching = true;
      touchY = event.touches[0].clientY;
      selectNearest();
    },
    { passive: false },
  );
  toggle.addEventListener(
    "touchmove",
    (event) => {
      if (!touching) return;
      event.preventDefault();
      touchY = event.touches[0].clientY;
    },
    { passive: false },
  );
  toggle.addEventListener(
    "touchend",
    (event) => {
      if (!touching) return;
      event.preventDefault();
      touching = false;
      cancelAnimationFrame(frame);
      const point = event.changedTouches[0];
      touchY = point.clientY;
      selectNearest();
      const target = active;
      const rect = list.getBoundingClientRect();
      const inside =
        point.clientX >= rect.left &&
        point.clientX <= rect.right &&
        point.clientY >= rect.top &&
        point.clientY <= rect.bottom;
      if (target && inside) {
        close();
        target.click();
      } else if (enteredList || closeOnTap) close();
    },
    { passive: false },
  );
  toggle.addEventListener("touchcancel", () => close());
}
