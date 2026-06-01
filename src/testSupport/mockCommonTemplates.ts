import commonTemplateCatalog from "../../assets/common_templates.json";
import type { CommonTemplate } from "../types";

export function commonTemplates(): CommonTemplate[] {
  return commonTemplateCatalog.templates.map((template) => ({
    id: template.id,
    display_name: template.display_name,
    language: template.language,
    source_path: `mock://${template.relative_path}`,
    source: `${template.source_lines.join("\n")}\n`
  }));
}
