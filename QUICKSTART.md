# 🚀 Debuggatha Quick Start

Get debugging with your AI witch in under 5 minutes!

## Step 1: Install Dependencies (1 minute)

```bash
cd /Users/ti/Desktop/Debuggatha
npm install
```

## Step 2: Compile TypeScript (30 seconds)

```bash
npm run compile
```

## Step 3: Get Gemini API Key (2 minutes)

1. Visit: https://makersuite.google.com/app/apikey
2. Sign in with Google account
3. Click "Create API Key"
4. Copy the key (starts with "AIza...")

## Step 4: Launch Extension (30 seconds)

1. Open this folder in VS Code
2. Press `F5` (or Run → Start Debugging)
3. A new VS Code window opens (Extension Development Host)

## Step 5: Configure API Key (30 seconds)

In the new window:
1. Press `Cmd+Shift+P` (or `Ctrl+Shift+P` on Windows)
2. Type: "Debuggatha: Set Gemini API Key"
3. Paste your API key
4. Press Enter

## Step 6: Start Debugging! (NOW!)

1. Look for the witch icon 🧙‍♀️ in the Activity Bar (left sidebar)
2. Click it to open Debuggatha
3. Choose a personality:
   - 😠 Angry Witch - Harsh but helpful
   - 😊 Kind Witch - Gentle and supportive  
   - 🧙‍♀️ Wise Witch - Professional architect
4. Type a question like:
   ```
   "Why is my async function not working?"
   ```
5. Click "✨ Invoke Witch"

## Example Workflow

```
1. Open a buggy file in VS Code
2. Open Debuggatha sidebar
3. Click "📁 Add Files to Context"
4. Select the buggy file
5. Type: "Find the bug in this code"
6. Get instant AI-powered debugging!
```

## Troubleshooting

**"Extension not found"**
- Make sure you pressed `F5` from the Debuggatha folder
- Check that `npm install` completed successfully

**"API key not found"**
- Run the "Set Gemini API Key" command again
- Make sure you copied the entire key

**"No response from AI"**
- Check your internet connection
- Verify your Gemini API key is valid
- Check for rate limits (wait a few seconds)

## Tips

💡 **Add context**: Select files to give the witch more info  
💡 **Be specific**: Describe exact errors, line numbers, behavior  
💡 **Try personalities**: Each witch has a unique perspective  
💡 **Check history**: Previous messages stay in the chat  

## Next Steps

- Read [README.md](README.md) for full documentation
- Check [IMPLEMENTATION.md](IMPLEMENTATION.md) for architecture details
- Modify `prompts/personalities.json` to customize personalities
- Edit `webview/styles.css` to change the look

---

**You're ready to debug with magic!** 🧙‍♀️✨
