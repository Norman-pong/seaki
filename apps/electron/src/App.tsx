import { useEffect, useState } from "react";

import { createElectronAppModel } from "./appModel";
import "./styles.css";
import type { ElectronAppModel } from "./appModel";

export function App() {
  const [model, setModel] = useState<ElectronAppModel>({
    importStage: "selected",
    workspaceStage: "initializing",
    workspaceTitle: "ws_local_preview",
  });

  useEffect(() => {
    let active = true;

    void createElectronAppModel().then((nextModel) => {
      if (active) {
        setModel(nextModel);
      }
    });

    return () => {
      active = false;
    };
  }, []);

  return (
    <main className="shell">
      <section className="statusPanel" aria-labelledby="workspace-title">
        <p className="label">seaki Electron MVP</p>
        <h1 id="workspace-title">{model.workspaceTitle}</h1>
        <dl>
          <div>
            <dt>Workspace</dt>
            <dd>{model.workspaceStage}</dd>
          </div>
          <div>
            <dt>Import</dt>
            <dd>{model.importStage}</dd>
          </div>
        </dl>
      </section>
    </main>
  );
}
