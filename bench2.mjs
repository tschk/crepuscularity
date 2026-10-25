import { JSDOM } from "jsdom";

const dom = new JSDOM(`<!DOCTYPE html><html><body><div id="container"></div></body></html>`);
const document = dom.window.document;

const container = document.getElementById("container");
for (let i = 0; i < 1000; i++) {
    const row = document.createElement("a");
    row.className = "downloads-crate-row";
    row.innerHTML = `<span><strong>Name</strong><small>1.0</small></span><b data-downloads-values="1,2,3">123</b>`;
    container.appendChild(row);
}

const rows = Array.from(container.children);

// Benchmark querySelector
console.time("querySelector");
for (let i = 0; i < 100; i++) {
    for (const row of rows) {
        const span = row.querySelector("span");
        const b = row.querySelector("b");
    }
}
console.timeEnd("querySelector");

// Benchmark firstElementChild / lastElementChild
console.time("elementChild");
for (let i = 0; i < 100; i++) {
    for (const row of rows) {
        const span = row.firstElementChild;
        const b = row.lastElementChild;
    }
}
console.timeEnd("elementChild");
