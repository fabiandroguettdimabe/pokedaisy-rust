import assert from "node:assert/strict";
import { chromium } from "../ui/node_modules/playwright-core/index.mjs";

const cdpUrl = process.env.VYNTRA_CORE_CHECK_CDP_URL;
if (!cdpUrl) throw new Error("VYNTRA_CORE_CHECK_CDP_URL is required");

const systems = [
  { index: 0, label: "Game Boy / Game Boy Color", library: "mGBA" },
  { index: 1, label: "Game Boy Advance", library: "mGBA" },
  { index: 2, label: "Nintendo DS", library: "melonDS DS" },
];

const browser = await chromium.connectOverCDP(cdpUrl);
try {
  const page = browser.contexts().flatMap((context) => context.pages())[0];
  assert.ok(page, "Vyntra WebView2 page is missing");
  await page.evaluate(() => {
    const originalAck = window.vyntraAck;
    window.vyntraAcks = [];
    window.vyntraAck = (ack) => {
      window.vyntraAcks.push(ack);
      originalAck(ack);
    };
  });
  await page.getByRole("complementary").getByRole("button", { name: "Núcleos", exact: true }).click();
  await page.getByRole("heading", { name: "Vyntra Core Manager" }).waitFor({ timeout: 10_000 });

  const results = [];
  for (const system of systems) {
    const row = page.locator(".setting").filter({
      has: page.getByRole("heading", { name: system.label, exact: true }),
    });
    const ackStart = await page.evaluate(() => window.vyntraAcks.length);
    await row.getByRole("button", { name: "Probar", exact: true }).click();
    await row.getByText("Carga verificada", { exact: true }).waitFor({ timeout: 15_000 });
    await page.waitForFunction(
      (start) => window.vyntraAcks.slice(start).some((ack) => ack.result?.core_probe),
      ackStart,
      { timeout: 15_000 },
    );
    const terminalAck = await page.evaluate(
      (start) => window.vyntraAcks.slice(start).find((ack) => ack.result?.core_probe),
      ackStart,
    );
    assert.equal(terminalAck.ok, true);
    assert.equal(terminalAck.result.core_probe.status, "loadable");
    assert.ok(terminalAck.command_id);
    const acceptedAck = await page.evaluate(
      ({ start, commandId }) =>
        window.vyntraAcks
          .slice(start)
          .some((ack) => ack.command_id === commandId && ack.ok && !ack.result),
      { start: ackStart, commandId: terminalAck.command_id },
    );
    assert.equal(acceptedAck, true, "terminal ACK must reuse the accepted command ID");
    const report = page.locator('[role="status"]').filter({ hasText: "Diagnóstico:" });
    await report.getByText(new RegExp(system.library.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))).waitFor({ timeout: 5_000 });
    const reportText = (await report.innerText()).replace(/\s+/g, " ");
    assert.match(reportText, /Diagnóstico: loadable/);
    results.push({
      system: system.label,
      status: "Carga verificada",
      command_id: terminalAck.command_id,
      diagnostic: reportText,
    });
  }

  console.log(JSON.stringify({ result: "PASS", probes: results }));
} finally {
  await browser.close();
}
