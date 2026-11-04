import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

// L'UI usa shadcn/ui + Tailwind: lo stack gluestack/react-native-web è stato
// rimosso perché incompatibile con il dev server Vite (schermata bianca).
const BANNED_UI_STACK = {
  patterns: [
    {
      group: ["*gluestack*", "react-native", "react-native-*", "react-native/*", "expo/html-elements"],
      message: "Stack gluestack/react-native non supportato: usa shadcn/ui + Tailwind.",
    },
  ],
};

export default tseslint.config(
  { ignores: ["dist/**", "node_modules/**", "src-tauri/**", "docs/**", "public/**", "src/bindings/**"] },
  js.configs.recommended,
  tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}", "vite.config.ts"],
    languageOptions: { globals: globals.browser },
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      "no-restricted-imports": ["error", BANNED_UI_STACK],
      // Validazioni di input e parser di sequenze ANSI usano caratteri di controllo di proposito.
      "no-control-regex": "off",
      "@typescript-eslint/no-unused-vars": ["error", { ignoreRestSiblings: true }],
    },
  },
  {
    files: ["tests/**/*.mjs", "scripts/**/*.mjs", "eslint.config.js"],
    languageOptions: { globals: globals.node },
  },
);
