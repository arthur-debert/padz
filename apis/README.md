# Padz API Wrappers

This directory contains language-specific API wrappers for the padz CLI tool. These wrappers provide a programmatic interface to padz functionality, making it easy to build editor plugins and other integrations.

## Available Wrappers

### TypeScript/JavaScript (`padz.ts`)
- Full TypeScript support with type definitions
- Promise-based async API
- Safe command execution using `execFile` (no shell injection)
- Supports custom working directory

Example:
```typescript
import { PadzClient } from './padz';

const client = new PadzClient({ cwd: '/my/project' });
const scratches = await client.list();
const scratch = await client.create('My new scratch content');
```

### Python (`padz.py`)
- Type hints and dataclasses
- Both class-based and module-level APIs
- Safe subprocess execution
- Custom exception handling

Example:
```python
import padz

# Module-level API
scratches = padz.list()
scratch = padz.create("My new scratch content")

# Or use the client
client = padz.PadzClient(cwd="/my/project")
scratches = client.list()
```

### Lua (`padz.lua`)
- Compatible with Neovim's Lua environment
- Works with standard Lua (requires JSON library)
- Safe command execution
- Both OOP and functional APIs

Example:
```lua
local padz = require('padz')

-- Functional API
local scratches = padz.list()
local scratch = padz.create("My new scratch content")

-- Or use the client
local client = padz.new({ cwd = "/my/project" })
local scratches = client:list()
```

## Features

All wrappers provide:
- ✅ Create, list, view, delete, search scratches
- ✅ Project and global scope support
- ✅ Safe from shell injection attacks
- ✅ Working directory management
- ✅ JSON parsing of responses
- ✅ Proper error handling

## Requirements

- padz CLI must be installed and available in PATH
- For Lua: Neovim or a JSON library for standard Lua
- For TypeScript: Node.js
- For Python: Python 3.7+

## Usage in Editor Plugins

These wrappers handle all the complexity of:
- Shell command execution
- JSON parsing
- Error handling
- Working directory management

Plugin authors just need to:
1. Import the appropriate wrapper
2. Create UI components (telescope picker, VSCode command, etc.)
3. Call the wrapper methods

Example Neovim plugin structure:
```lua
-- telescope-padz.nvim
local padz = require('padz')
local pickers = require('telescope.pickers')

function M.list_scratches()
  local scratches = padz.list()
  -- Create telescope picker with scratches
end
```