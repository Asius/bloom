import { GithubIcon } from "../icons";
import { invoke } from "@tauri-apps/api/core";
import { Check, Circle, ExternalLink, LoaderCircle, RefreshCw, Settings2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { PanelHeader } from "./PanelHeader";
import "./IslandGithub.css";
type Run = {
	id: number;
	name: string;
	path: string;
	status: string;
	conclusion: string | null;
	head_sha: string;
	updated_at: string;
	html_url: string;
};
type Job = {
	id: number;
	name: string;
	status: string;
	conclusion: string | null;
	steps?: { name: string; status: string; conclusion: string | null }[];
};
type Snapshot = { repository: string; runs: Run[] };
function state(status: string, conclusion: string | null): string {
	if (status !== "completed") return status === "queued" ? "En cola" : "En curso";
	return (
		(
			{
				success: "Correcto",
				failure: "Falló",
				cancelled: "Cancelado",
				skipped: "Omitido",
				timed_out: "Tiempo agotado",
				action_required: "Requiere acción"
			} as Record<string, string>
		)[conclusion || ""] || "Finalizado"
	);
}
export function IslandGithub({ previous, next }: { previous: () => void; next: () => void }) {
	const [editing, setEditing] = useState(false);
	const [showOther, setShowOther] = useState(false);
	const [repository, setRepository] = useState("Asius/bloom");
	const [input, setInput] = useState("Asius/bloom");
	const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
	const [error, setError] = useState("");
	const [updated, setUpdated] = useState<number | null>(null);
	const [selected, setSelected] = useState<number | null>(null);
	const [jobs, setJobs] = useState<Job[]>([]);
	const [jobError, setJobError] = useState("");
	const [jobsLoaded, setJobsLoaded] = useState(false);
	const generation = useRef(0);
	const jobGeneration = useRef(0);
	useEffect(() => {
		void invoke<Record<string, string>>("load_settings")
			.then((s) => {
				const v = s["bloom-github-repository"];
				if (v) {
					setInput(v);
					setRepository(v);
				}
			})
			.catch(() => {});
	}, []);
	const refresh = useCallback(async () => {
		const request = ++generation.current;
		try {
			const result = await invoke<Snapshot>("github_monitor_snapshot", { repository });
			if (request !== generation.current) return;
			setSnapshot(result);
			setUpdated(Date.now());
			setError("");
		} catch (e) {
			if (request === generation.current) setError(String(e));
		}
	}, [repository]);
	useEffect(() => {
		setSnapshot(null);
		setSelected(null);
		setJobs([]);
		setUpdated(null);
		void refresh();
		const timer = setInterval(() => void refresh(), 120000);
		return () => {
			clearInterval(timer);
			generation.current++;
			jobGeneration.current++;
		};
	}, [refresh]);
	const selectRepository = async () => {
		const value = input.trim();
		if (!/^[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+$/.test(value)) {
			setError("Usa el formato propietario/repositorio.");
			return;
		}
		try {
			await invoke("save_setting", { key: "bloom-github-repository", value });
			setRepository(value);
			setEditing(false);
			setError("");
		} catch {
			setError("No se pudo guardar el repositorio.");
		}
	};
	useEffect(() => {
		const request = ++jobGeneration.current;
		if (selected === null) return;
		setJobs([]);
		setJobsLoaded(false);
		setJobError("");
		void invoke<{ jobs: Job[] }>("github_monitor_jobs", { repository, runId: selected })
			.then((result) => {
				if (request === jobGeneration.current) {
					setJobs(result.jobs);
					setJobsLoaded(true);
				}
			})
			.catch((e) => {
				if (request === jobGeneration.current) setJobError(String(e));
			});
		return () => {
			jobGeneration.current++;
		};
	}, [repository, selected, updated]);
	const names = ["CI", "CodeQL", "Release"];
	for (const run of snapshot?.runs ?? []) {
		if (
			run.name &&
			!names.some((name) => name.toLowerCase() === run.name.toLowerCase()) &&
			names.length < 8
		)
			names.push(run.name);
	}
	const rows = names.map((name) => ({
		name,
		run: snapshot?.runs.find((r) => r.name.toLowerCase() === name.toLowerCase())
	}));
	return (
		<section
			className="github-panel"
			aria-label="Seguimiento de GitHub"
			onClick={(e) => e.stopPropagation()}
		>
			<PanelHeader
				title={`GitHub · ${repository.toLowerCase() === "asius/bloom" ? "Bloom" : repository.split("/")[1]}`}
				icon={<GithubIcon size={25} />}
				subtitle={repository}
				previous={previous}
				next={next}
				actions={
					<>
						<button
							title="Cambiar repositorio"
							aria-label="Cambiar repositorio"
							onClick={() => setEditing((v) => !v)}
						>
							<Settings2 size={16} />
						</button>
						<button title="Actualizar" aria-label="Actualizar" onClick={() => void refresh()}>
							<RefreshCw size={16} />
						</button>
						<button
							className="github-open"
							onClick={() =>
								void invoke("github_monitor_open", { repository }).catch((e) => setError(String(e)))
							}
						>
							Abrir en GitHub <ExternalLink size={15} />
						</button>
					</>
				}
			/>
			<div className="github-content">
				{editing && (
					<div className="github-repository">
						<label htmlFor="github-repository">Repositorio público</label>
						<div>
							<input
								id="github-repository"
								value={input}
								onChange={(e) => setInput(e.target.value)}
								onKeyDown={(e) => {
									if (e.key === "Enter") void selectRepository();
								}}
							/>
							<button onClick={() => void selectRepository()}>Seleccionar</button>
							<button title="Actualizar" aria-label="Actualizar" onClick={() => void refresh()}>
								<RefreshCw size={15} />
							</button>
							<button
								title="Abrir en GitHub"
								aria-label="Abrir en GitHub"
								onClick={() =>
									void invoke("github_monitor_open", { repository }).catch((e) =>
										setError(String(e))
									)
								}
							>
								<ExternalLink size={15} />
							</button>
						</div>
					</div>
				)}
				{error && (
					<p className="github-error" role="alert">
						{error}
						{updated ? " · Mostrando datos anteriores" : ""}
					</p>
				)}
				{!snapshot && !error && <p>Consultando GitHub…</p>}
				{snapshot &&
					rows
						.filter(({ name }) => showOther || ["CI", "CodeQL", "Release"].includes(name))
						.map(({ name, run }) => (
							<div className="github-run" key={name}>
								<button
									disabled={!run}
									onClick={() =>
										run && setSelected((current) => (current === run.id ? null : run.id))
									}
									aria-expanded={!!run && selected === run.id}
								>
									<span
										className={`github-state ${run?.conclusion === "success" ? "success" : run?.conclusion === "failure" ? "failure" : ""}`}
									>
										{run?.status && run.status !== "completed" ? (
											<LoaderCircle className="github-spin" size={21} />
										) : run?.conclusion === "success" ? (
											<Check size={15} />
										) : run?.conclusion === "failure" ? (
											<X size={15} />
										) : (
											<Circle size={19} />
										)}
									</span>
									<strong>{name}</strong>
									<span>{run ? state(run.status, run.conclusion) : "Sin ejecución reciente"}</span>
									{run && <small>{run.head_sha?.slice(0, 7)}</small>}
								</button>
								{run && selected === run.id && (
									<div className="github-jobs">
										{jobError ||
											(jobs.length
												? jobs.map((job) => (
														<div key={job.id}>
															<strong>{job.name}</strong> · {state(job.status, job.conclusion)}
															{job.steps?.map((step, index) => (
																<div key={index}>
																	{step.name} · {state(step.status, step.conclusion)}
																</div>
															))}
														</div>
													))
												: jobsLoaded
													? "Sin trabajos disponibles"
													: "Cargando pasos…")}
									</div>
								)}
							</div>
						))}
				<div className="github-verification">
					<Circle size={21} />
					<strong>Canal y firma</strong>
					<span>Sin comprobar</span>
				</div>
				{names.length > 3 && (
					<button className="github-other" onClick={() => setShowOther((v) => !v)}>
						{showOther ? "Ocultar otras ejecuciones" : "Otras ejecuciones"}
					</button>
				)}
				<p className="github-foot">
					Consulta pública
					{updated && ` · Actualizado ${new Date(updated).toLocaleTimeString()}`}
				</p>
				{editing && (
					<p className="github-foot">
						Cuenta GitHub: conexión pendiente de configurar. Los repositorios privados aún no están
						disponibles.
					</p>
				)}
			</div>
		</section>
	);
}
