import readline from "node:readline";
import { chromium } from "playwright";

const DEFAULT_URL = "http://127.0.0.1:4173/admin";
const DEFAULT_PROFILE_DIR = "/tmp/fumen-playwright-live-profile";
const DEFAULT_VIEWPORT = { width: 390, height: 844 };

function parseArgs(argv) {
    const args = {
        url: DEFAULT_URL,
        profileDir: DEFAULT_PROFILE_DIR,
        viewport: DEFAULT_VIEWPORT,
        mobile: true,
        headless: false,
    };

    for (let i = 0; i < argv.length; i += 1) {
        const value = argv[i];
        if (value === "--url" && argv[i + 1]) {
            args.url = argv[i + 1];
            i += 1;
        } else if (value === "--profile-dir" && argv[i + 1]) {
            args.profileDir = argv[i + 1];
            i += 1;
        } else if (value === "--viewport" && argv[i + 1]) {
            const [width, height] = argv[i + 1].split("x").map((part) => Number(part));
            if (Number.isFinite(width) && Number.isFinite(height)) {
                args.viewport = { width, height };
            }
            i += 1;
        } else if (value === "--desktop") {
            args.mobile = false;
        } else if (value === "--headless") {
            args.headless = true;
        }
    }

    return args;
}

async function dumpState(page) {
    const data = await page.evaluate(() => {
        const cards = Array.from(
            document.querySelectorAll(".admin-user-list .music-card"),
        ).map((el) => {
            const rect = el.getBoundingClientRect();
            return {
                top: rect.top,
                bottom: rect.bottom,
                left: rect.left,
                right: rect.right,
                text: (el.textContent || "").trim().slice(0, 80),
            };
        });

        return {
            url: location.href,
            title: document.title,
            bodyText: document.body.textContent?.trim().slice(0, 220) || "",
            count: cards.length,
            cards,
            bodyScrollHeight: document.body.scrollHeight,
            viewportHeight: window.innerHeight,
        };
    });

    console.log(JSON.stringify(data, null, 2));
}

async function main() {
    const args = parseArgs(process.argv.slice(2));
    const context = await chromium.launchPersistentContext(args.profileDir, {
        headless: args.headless,
        viewport: args.viewport,
        isMobile: args.mobile,
        hasTouch: args.mobile,
    });

    const page = context.pages()[0] ?? (await context.newPage());
    await page.goto(args.url, { waitUntil: "networkidle" });

    console.log("READY");
    console.log(`Profile: ${args.profileDir}`);
    console.log(`URL: ${args.url}`);
    console.log("Commands: state | url | goto <url> | reload | screenshot <file> | quit");

    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout,
    });

    rl.on("line", async (line) => {
        const trimmed = line.trim();
        try {
            if (trimmed === "state") {
                await dumpState(page);
                return;
            }

            if (trimmed === "url") {
                console.log(await page.url());
                return;
            }

            if (trimmed === "reload") {
                await page.reload({ waitUntil: "networkidle" });
                console.log("reloaded");
                return;
            }

            if (trimmed.startsWith("goto ")) {
                const targetUrl = trimmed.slice("goto ".length).trim();
                if (targetUrl) {
                    await page.goto(targetUrl, { waitUntil: "networkidle" });
                    console.log(`navigated to ${targetUrl}`);
                }
                return;
            }

            if (trimmed.startsWith("screenshot ")) {
                const path = trimmed.slice("screenshot ".length).trim();
                if (path) {
                    await page.screenshot({ path, fullPage: true });
                    console.log(`saved ${path}`);
                }
                return;
            }

            if (trimmed === "quit") {
                rl.close();
                await context.close();
                process.exit(0);
                return;
            }

            if (trimmed) {
                console.log(`unknown command: ${trimmed}`);
            }
        } catch (error) {
            console.error(error);
        }
    });

    await new Promise(() => {});
}

main().catch((error) => {
    console.error(error);
    process.exit(1);
});
