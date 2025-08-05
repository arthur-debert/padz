-- padz.lua - Lua API for padz CLI
-- Compatible with Neovim's Lua environment

local M = {}

-- Helper to check if we're in Neovim
local is_neovim = vim and vim.fn and vim.fn.system

-- Execute command based on environment
local function execute_command(cmd)
  if is_neovim then
    -- Neovim: vim.fn.system handles escaping when given a list
    local result = vim.fn.system(cmd)
    local exit_code = vim.v.shell_error
    return result, exit_code
  else
    -- Standard Lua: use io.popen (less safe but works)
    local handle = io.popen(table.concat(cmd, ' ') .. ' 2>&1')
    local result = handle:read('*a')
    local success = handle:close()
    return result, success and 0 or 1
  end
end

-- JSON parsing based on environment
local function parse_json(str)
  if is_neovim then
    return vim.json.decode(str)
  else
    -- For standard Lua, you'll need a JSON library like lunajson or cjson
    -- This is a placeholder - replace with your JSON library of choice
    error("JSON parsing not available. Please install a JSON library.")
  end
end

-- JSON encoding based on environment
local function encode_json(obj)
  if is_neovim then
    return vim.json.encode(obj)
  else
    -- For standard Lua, you'll need a JSON library
    error("JSON encoding not available. Please install a JSON library.")
  end
end

-- PadzClient class
local PadzClient = {}
PadzClient.__index = PadzClient

function PadzClient:new(opts)
  opts = opts or {}
  local self = setmetatable({}, PadzClient)
  self.cwd = opts.cwd
  return self
end

-- Execute padz command with proper error handling
function PadzClient:_exec(args)
  local cmd = {'padz'}
  for _, arg in ipairs(args) do
    table.insert(cmd, arg)
  end

  -- Handle cwd if specified
  local old_cwd
  if self.cwd and is_neovim then
    old_cwd = vim.fn.getcwd()
    vim.cmd('cd ' .. vim.fn.fnameescape(self.cwd))
  end

  local output, exit_code = execute_command(cmd)

  -- Restore cwd
  if old_cwd then
    vim.cmd('cd ' .. vim.fn.fnameescape(old_cwd))
  end

  -- Handle errors
  if exit_code ~= 0 then
    local ok, parsed = pcall(parse_json, output)
    if ok and parsed.error then
      error(parsed.error)
    else
      error("Command failed: " .. output)
    end
  end

  -- Check for JSON error in successful response
  local ok, parsed = pcall(parse_json, output)
  if ok and parsed.error then
    error(parsed.error)
  end

  return output
end

-- Create a new scratch from content
function PadzClient:create(content)
  if not content or content == '' then
    error("Content cannot be empty")
  end

  -- For create, we need to pipe content to stdin
  -- This is more complex and environment-specific
  local cmd = {'padz', '--format', 'json'}
  
  if is_neovim then
    -- Use vim.fn.system with input
    local old_cwd
    if self.cwd then
      old_cwd = vim.fn.getcwd()
      vim.cmd('cd ' .. vim.fn.fnameescape(self.cwd))
    end

    local result = vim.fn.system(cmd, content)
    local exit_code = vim.v.shell_error

    if old_cwd then
      vim.cmd('cd ' .. vim.fn.fnameescape(old_cwd))
    end

    if exit_code ~= 0 then
      local ok, parsed = pcall(parse_json, result)
      if ok and parsed.error then
        error(parsed.error)
      else
        error("Failed to create scratch")
      end
    end

    return parse_json(result)
  else
    -- Standard Lua: use io.popen with write
    local cmd_str = table.concat(cmd, ' ')
    local handle = io.popen(cmd_str, 'w')
    handle:write(content)
    handle:close()
    
    -- Read the output separately
    local output_handle = io.popen(cmd_str .. ' 2>&1', 'r')
    local result = output_handle:read('*a')
    output_handle:close()
    
    return parse_json(result)
  end
end

-- List all scratches
function PadzClient:list(opts)
  opts = opts or {}
  local args = {'ls', '--format', 'json'}
  if opts.all then table.insert(args, '--all') end
  if opts.global then table.insert(args, '--global') end

  local output = self:_exec(args)
  return parse_json(output)
end

-- View scratch content
function PadzClient:view(index, opts)
  opts = opts or {}
  local args = {'view', tostring(index), '--format', 'json'}
  if opts.all then table.insert(args, '--all') end
  if opts.global then table.insert(args, '--global') end

  local output = self:_exec(args)
  local result = parse_json(output)
  return result.content
end

-- Open scratch in editor
function PadzClient:open(index, opts)
  opts = opts or {}
  local args = {'open', tostring(index), '--format', 'json'}
  if opts.all then table.insert(args, '--all') end

  self:_exec(args)
end

-- Peek at scratch content
function PadzClient:peek(index, opts)
  opts = opts or {}
  local args = {'peek', tostring(index), '--format', 'json'}
  if opts.all then table.insert(args, '--all') end
  if opts.global then table.insert(args, '--global') end
  if opts.lines then
    table.insert(args, '--lines')
    table.insert(args, tostring(opts.lines))
  end

  local output = self:_exec(args)
  local result = parse_json(output)
  return result.content
end

-- Delete a scratch
function PadzClient:delete(index, opts)
  opts = opts or {}
  local args = {'delete', tostring(index), '--format', 'json'}
  if opts.all then table.insert(args, '--all') end

  self:_exec(args)
end

-- Get scratch file path
function PadzClient:path(index, opts)
  opts = opts or {}
  local args = {'path', tostring(index), '--format', 'json'}
  if opts.all then table.insert(args, '--all') end

  local output = self:_exec(args)
  local result = parse_json(output)
  return result.path
end

-- Search scratches
function PadzClient:search(term, opts)
  opts = opts or {}
  local args = {'search', term, '--format', 'json'}
  if opts.all then table.insert(args, '--all') end
  if opts.global then table.insert(args, '--global') end

  local output = self:_exec(args)
  return parse_json(output)
end

-- Cleanup old scratches
function PadzClient:cleanup(days)
  local args = {'cleanup', '--format', 'json'}
  if days then
    table.insert(args, '--days')
    table.insert(args, tostring(days))
  end

  self:_exec(args)
end

-- Nuke scratches
function PadzClient:nuke(opts)
  opts = opts or {}
  local args = {'nuke', '--format', 'json'}
  if opts.all then table.insert(args, '--all') end
  if opts.yes then table.insert(args, '--yes') end

  local output = self:_exec(args)
  return parse_json(output)
end

-- Create a new client with different cwd
function PadzClient:with_cwd(cwd)
  return PadzClient:new({ cwd = cwd })
end

-- Module functions for direct use
function M.new(opts)
  return PadzClient:new(opts)
end

-- Convenience functions using default client
local default_client = PadzClient:new()

function M.create(content)
  return default_client:create(content)
end

function M.list(opts)
  return default_client:list(opts)
end

function M.view(index, opts)
  return default_client:view(index, opts)
end

function M.open(index, opts)
  return default_client:open(index, opts)
end

function M.peek(index, opts)
  return default_client:peek(index, opts)
end

function M.delete(index, opts)
  return default_client:delete(index, opts)
end

function M.path(index, opts)
  return default_client:path(index, opts)
end

function M.search(term, opts)
  return default_client:search(term, opts)
end

function M.cleanup(days)
  return default_client:cleanup(days)
end

function M.nuke(opts)
  return default_client:nuke(opts)
end

return M