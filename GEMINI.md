# Git Commit Rules

- **Do not automatically commit changes.** 
- Always wait for the user to explicitly say "commit" before running `git commit`. 
- When the user says "commit", it means they have reviewed the changes and approved them.

# Workspace Rules

- **Temporary Files**: When creating scratch scripts, API test scripts, or any temporary files, ALWAYS place them strictly inside the **current working directory's** `./.temp/` folder. Do NOT place them in the global `~/Temp` folder, the parent directory, or the project root.
