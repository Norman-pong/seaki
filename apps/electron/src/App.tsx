import { createElectronAppModel } from "./appModel";
import "./styles.css";

export function App() {
  const model = createElectronAppModel();

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
