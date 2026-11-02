import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { IslandNotifications } from "../src/components/IslandNotifications";
import type { NotificationStatus } from "../src/hooks/useNotifications";

function panel(status: NotificationStatus, timeFormat24h = false) {
	return renderToStaticMarkup(
		<IslandNotifications
			status={status}
			time="10:24"
			timeFormat24h={timeFormat24h}
			items={[{ id: 1, app: "Correo", title: "Contenido privado", body: "Detalle", createdMs: 0 }]}
			requestAccess={() => {}}
			setActive={() => {}}
			preferenceError={null}
			preferenceBusy={false}
			previous={() => {}}
			next={() => {}}
		/>
	);
}

test("el panel enseña contenido disponible con permiso", () => {
	expect(panel("allowed")).toContain("Contenido privado");
});

test("el panel denegado no muestra contenido aunque reciba una lista obsoleta", () => {
	const html = panel("denied");
	expect(html).toContain("desactivado en Windows");
	expect(html).not.toContain("Contenido privado");
});

test("desactivado ofrece activación y no expone contenidos obsoletos", () => {
	const html = panel("disabled");
	expect(html).toContain("Activar");
	expect(html).not.toContain("Contenido privado");
});

test("las horas de los avisos respetan el formato de 24 horas elegido", () => {
	const html = panel("allowed", true);
	const expectedTime = new Date(0).toLocaleTimeString([], {
		hour: "2-digit",
		minute: "2-digit",
		hour12: false
	});
	expect(html).toContain(expectedTime);
});
