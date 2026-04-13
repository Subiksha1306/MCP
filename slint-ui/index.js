import * as slint from "slint-ui";
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

// Fix for ES module path resolution
const __dirname = dirname(fileURLToPath(import.meta.url));

// Load the Slint UI
const slintFile = join(__dirname, "main.slint");
const ui = slint.loadFile(slintFile);

// Initialize the UI
// Assuming MainWindow is the exported component name from main.slint
const main = new ui.MainWindow();

// UI State
let searchResults = [];
let chatMessages = [];
let serverRunning = false;

// UI Callbacks
main.search_documents = async (query) => {
    console.log('Search triggered:', query);
    // In standalone mode, we could call the local MCP server via HTTP
    // but for now we'll just log
};

main.send_chat_message = async (message) => {
    console.log('Chat message:', message);
};

// Start the UI
main.run();

console.log("🚀 Slint UI running standalone...");