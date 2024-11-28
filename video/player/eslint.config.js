// @ts-check

import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import prettier from "eslint-config-prettier";
import globals from "globals";

export default [
	eslint.configs.recommended,
	prettier,
	{
		languageOptions: {
			globals: {
				...globals.browser,
			},
		},
	},
	...tseslint.configs.recommended,
];
