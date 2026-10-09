import { formatFileFormat } from "../../format/fileFormat";
import { formatFileSize } from "../../format/fileSize";
import { getTranslations } from "../../i18n";
import { useAppState } from "../../state";
import { useSelectRow } from "../details/useSelectRow";
import { ItemRowStatus } from "./ItemRowStatus";
import "./ItemTable.css";
import { MetadataChips } from "./MetadataChips";

export function ItemTable() {
  const { language, items, job, selectedId } = useAppState();
  const { selectRow } = useSelectRow();
  const t = getTranslations(language.language);

  return (
    <div className="item-table-container">
      <table className="item-table">
        <thead>
          <tr>
            <th>{t.itemList.headerName}</th>
            <th style={{ width: 56 }}>{t.itemList.headerFormat}</th>
            <th style={{ width: 72, textAlign: "right" }}>
              {t.itemList.headerSize}
            </th>
            <th>{t.itemList.headerKinds}</th>
            <th style={{ width: 170 }}>{t.itemList.headerStatus}</th>
          </tr>
        </thead>
        <tbody>
          {items.map((item) => {
            const isSelected = item.id === selectedId;
            return (
              // The whole row selects; the name stays a button so the row
              // can be reached and picked from the keyboard, and its click
              // reaches the row like any other.
              <tr
                key={item.id}
                className={isSelected ? "selected" : ""}
                onClick={() => void selectRow(item.id)}
              >
                <td className="cell-name">
                  <button
                    type="button"
                    className="row-btn"
                    aria-pressed={isSelected}
                  >
                    {item.name}
                  </button>
                </td>
                <td className="mono">
                  {formatFileFormat(item.name, item.format)}
                </td>
                <td
                  className="mono"
                  style={{ textAlign: "right", whiteSpace: "nowrap" }}
                >
                  {formatFileSize(item.bytes)}
                </td>
                <td>
                  <MetadataChips
                    kinds={item.kinds}
                    hasError={item.error !== null}
                    language={language.language}
                  />
                </td>
                <ItemRowStatus
                  item={item}
                  job={job}
                  language={language.language}
                />
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
