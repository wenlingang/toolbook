import { expect, open, test } from "nsh/test";

// Run with TOOLBOOK_HOME pointing at an empty folder: the first launch seeds it.
// The tests rename, move and trash notes: never in the user's own library.
// A headless run can ask the app's own module where that is; a real
// platform's test file cannot load it.
const native = await import("toolbook/native").catch(() => null);
if (native?.Library.root().replace(/\/+$/, "").endsWith("/Toolbook")) {
  throw new Error("TOOLBOOK_HOME 必须指向一个空的临时目录：TOOLBOOK_HOME=$(mktemp -d)/lib nsh test .");
}
const desktop = { width: 1200, height: 800, platform: "macos" as const };

const sees = (app: { texts(): string[] }, part: string) => app.texts().some((t) => t.includes(part));

test("first launch seeds the library", async () => {
  const app = await open("./App.tsx", desktop);
  expect(app.texts()).toContain("Toolbook");
  expect(app.texts()).toContain("排查登录-401");
  expect(app.texts()).toContain("csv-to-sql");
});

test("the sidebar opens a note", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("排查登录-401");
  await app.waitUntil(() => sees(app, '"unix": 1761703600'));
});

test("the toggle shows the Markdown view", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("排查登录-401");
  await app.press("Markdown");
  await app.waitUntil(() => sees(app, '→ {"unix":1761703600'));
  await app.press("文档");
  await app.waitUntil(() => sees(app, '"unix": 1761703600'));
});

test("quick open filters notes", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("⌘P 打开");
  // Before filtering the overlay lists every note: each name shows in the sidebar and in it.
  expect(app.count("text:webhook-验签")).toBe(2);
  await app.type("text:搜索笔记", "trace");
  expect(app.count("text:webhook-验签")).toBe(1);
  expect(app.count("text:trace-慢请求")).toBe(2);
  await app.press("id:open-trace-慢请求");
  await app.waitUntil(() => sees(app, "mysql INSERT orders 164ms"));
});

test("escape closes quick open", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("⌘P 打开");
  await app.type("text:搜索笔记", "tr");
  expect(app.count("text:取消")).toBe(1);
  await app.keyboard.press("escape");
  expect(app.count("text:取消")).toBe(0);
});

test("the sidebar shows the app icon", async () => {
  const app = await open("./App.tsx", desktop);
  expect(app.count("label:Toolbook 图标")).toBe(1);
});

test("the sidebar filters notes by name", async () => {
  const app = await open("./App.tsx", desktop);
  expect(app.count("text:webhook-验签")).toBe(1);
  await app.type("text:筛选笔记", "csv");
  expect(app.count("text:webhook-验签")).toBe(0);
  expect(app.count("text:csv-to-sql")).toBeGreaterThan(0);
});

test("the preview shows the shared page with its actions", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("预览");
  expect(app.count("id:preview")).toBe(1);
  expect(app.count("text:导出 HTML…")).toBe(1);
  expect(app.count("text:复制 Markdown（含结果）")).toBe(1);
});

test("a new category gets notes from its plus button", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("＋ 新建");
  await app.chooseMenuItem("新建分类");
  await app.type("text:分类名", "接口调试\n");
  expect(app.count("text:接口调试")).toBe(1);
  await app.press("id:add-接口调试");
  await app.waitUntil(() => sees(app, "基础用法"));
  expect(app.count("text:未命名")).toBeGreaterThan(0);
});

test("escape cancels a new category", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("＋ 新建");
  await app.chooseMenuItem("新建分类");
  expect(app.count("text:分类名")).toBe(1);
  await app.keyboard.press("escape");
  expect(app.count("text:分类名")).toBe(0);
});

test("the plus menu makes a note", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("＋ 新建");
  await app.chooseMenuItem("新建笔记");
  await app.waitUntil(() => sees(app, "基础用法"));
});

test("a note is renamed from its context menu", async () => {
  const app = await open("./App.tsx", desktop);
  await app.rightClick("csv-to-sql");
  await app.chooseMenuItem("重命名");
  await app.clear("id:rename-input");
  await app.type("id:rename-input", "sql生成\n");
  expect(app.count("text:sql生成")).toBeGreaterThan(0);
  expect(app.count("text:csv-to-sql")).toBe(0);
});

test("a double click on the title renames the note", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("时区换算");
  await app.press("id:note-title");
  await app.press("id:note-title");
  await app.clear("id:rename-input");
  await app.type("id:rename-input", "时区\n");
  // In the sidebar and the top bar.
  expect(app.count("text:时区")).toBe(2);
  expect(app.count("text:时区换算")).toBe(0);
});

test("a note moves to another category", async () => {
  const app = await open("./App.tsx", desktop);
  await app.rightClick("trace-慢请求");
  await app.chooseMenuItem("移动到 > 签名");
  const texts = app.texts();
  expect(texts.indexOf("trace-慢请求")).toBeGreaterThan(texts.indexOf("签名"));
});

test("trashing a note says what happened", async () => {
  // Headless has no file manager, so the Trash answers an error there; on a
  // Mac the note goes to the Trash. Either way the window says so.
  const app = await open("./App.tsx", desktop);
  await app.rightClick("webhook-验签");
  await app.chooseMenuItem("移到废纸篓");
  await app.waitUntil(() => sees(app, "已将 webhook-验签 移到废纸篓") || sees(app, "no file dialogs, file manager"));
});

test("categories and notes are dragged into place", async () => {
  const app = await open("./App.tsx", desktop);
  await app.press("＋ 新建");
  await app.chooseMenuItem("新建分类");
  await app.type("text:分类名", "拖放目标\n");
  // The new category comes last; dragged far up, it becomes the first.
  await app.drag("id:dir-拖放目标", { y: -2000 });
  let texts = app.texts();
  expect(texts.indexOf("拖放目标")).toBeLessThan(texts.indexOf("数据转换"));
  // The order is kept.
  const again = await open("./App.tsx", desktop);
  texts = again.texts();
  expect(texts.indexOf("拖放目标")).toBeLessThan(texts.indexOf("数据转换"));
  // A note dragged far up lands in the first category.
  await again.drag("id:note-排查登录-401", { y: -2000 });
  texts = again.texts();
  const at = texts.indexOf("排查登录-401");
  expect(at).toBeGreaterThan(texts.indexOf("拖放目标"));
  expect(at).toBeLessThan(texts.indexOf("数据转换"));
});

test("a note dragged out can be dragged back", async () => {
  const app = await open("./App.tsx", desktop);
  await app.drag("id:note-trace-慢请求", { y: -2000 });
  let texts = app.texts();
  const first = texts.indexOf("trace-慢请求");
  // Far up: the first category. Then far down: the last one.
  await app.drag("id:note-trace-慢请求", { y: 2000 });
  texts = app.texts();
  const headers = texts.filter((t, i) => texts[i + 1] === "＋");
  const lastHeader = texts.lastIndexOf(headers[headers.length - 1]);
  expect(texts.indexOf("trace-慢请求")).toBeGreaterThan(lastHeader);
  expect(texts.indexOf("trace-慢请求")).not.toBe(first);
});

test("a note dragged a little goes into the category just above", async () => {
  const app = await open("./App.tsx", desktop);
  let texts = app.texts();
  const headers = texts.filter((t, i) => texts[i + 1] === "＋");
  const last = headers[headers.length - 1];
  const above = headers[headers.length - 2];
  // The first note of the last category, dragged up past that category's
  // header (a header and a row are about 30pt): into the one above.
  const first = texts[texts.indexOf(last) + 2];
  await app.drag(`id:note-${first}`, { y: -75 });
  texts = app.texts();
  const at = texts.indexOf(first);
  expect(at).toBeGreaterThan(texts.indexOf(above));
  expect(at).toBeLessThan(texts.indexOf(last));
});
