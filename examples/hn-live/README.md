# Hacker News, top 5: a site written in Tez that rebuilds itself

A small, independent site: the top five Hacker News stories, written entirely in [Tez](https://github.com/mryesiller/tez-lang).
No server, no framework, no JavaScript written by hand: the page is `main.tez`, compiled to WebAssembly and published as one HTML file.

## Two kinds of cron

1. **Build time (server side, every 5 minutes).** A scheduled job runs `build.mjs`: it fetches the current top five
   stories from the Hacker News API, writes them into `stories.tez`, and lets the Tez compiler **prove and test**
   `main.tez` (`tez check`, `tez test`), then compiles the page (`tez site`) and deploys it. A page that does not pass is
   never deployed; the previous one stays up. So the page opens with fresh content, without a request to anyone.
2. **Run time (in the visitor's browser).** While the page is open it follows the wall clock (`Sub.clock`): at every
   5-minute mark (hh:00, hh:05, ...) it asks the API again (`Fetch.json`) and redraws. The schedule is ordinary pure code
   in `update` (`slotOf(t) != m.slot`), covered by tests.

## Files

| File | What |
|---|---|
| `main.tez` | the page: model, schedule, requests, JSON reading, view, 18 tests |
| `story.tez` | the `Story` type |
| `stories.tez` | the stories baked in by the last build (rewritten by `build.mjs`) |
| `build.mjs` | the cron job's build step |
| `style.css` | the look |
| `workflow/build.yml` | the GitHub Actions workflow (copy to `.github/workflows/`) |

## Try it

```bash
npm install -g tez-lang          # the Tez compiler (WebAssembly); needs Node 22
node build.mjs                   # fetch, bake, prove, test, compile -> dist/index.html
npx serve dist                   # or any static server; open the page
```

Without installing: `TEZ="node /path/to/tez-lang/npm/tez-lang/bin/tez.mjs" node build.mjs`.

## Publish it as its own site

1. Create a new GitHub repository and put this folder's files in its root (and `workflow/build.yml` at `.github/workflows/build.yml`).
2. Settings, Pages, Source: **GitHub Actions**.
3. Run the workflow once (Actions, "Rebuild and deploy", Run workflow). From then on it runs every 5 minutes.

## What to know

- GitHub's scheduler is best effort: 5 minutes is its minimum and runs are often a few minutes late. That is why the page
  also refreshes itself in the browser.
- GitHub pauses scheduled workflows in a public repository after 60 days without any repository activity; re-enable it in
  the Actions tab (or push a commit).
- The page can reach one origin besides its own: `https://hacker-news.firebaseio.com` (`--allow` in `build.mjs`), enforced
  by the page's Content-Security-Policy and by the Tez host. Titles are drawn as text, never as HTML.
- The JSON is read by the language's own `Json` module (`Json.parse`, `j.get("title")`, ...): see `storyOf` in `main.tez`.
