import { GithubIcon } from "../icons";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	Check,
	CheckCheck,
	Circle,
	Copy,
	ExternalLink,
	LoaderCircle,
	LogIn,
	LogOut,
	RefreshCw,
	Settings2,
	ShieldCheck,
	X
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import {
	getGithubDeviceFlowState,
	startGithubDeviceFlow,
	subscribeGithubDeviceFlow
} from "../githubDeviceFlow";
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
type RepoChoice = { id: number; fullName: string; private: boolean; selected: boolean };
type GithubAccount = { connected: boolean; login: string | null; selected: RepoChoice[] };
type ReleaseVerification = {
	version: string;
	verified: boolean;
	bytes: number;
	detail: string;
};
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
	const [account, setAccount] = useState<GithubAccount>({
		connected: false,
		login: null,
		selected: []
	});
	const [repoChoices, setRepoChoices] = useState<RepoChoice[]>([]);
	const [authBusy, setAuthBusy] = useState(false);
	const [authMessage, setAuthMessage] = useState("");
	const [deviceFlow, setDeviceFlow] = useState(getGithubDeviceFlowState);
	const [codeCopied, setCodeCopied] = useState(false);
	const [verification, setVerification] = useState<ReleaseVerification | null>(null);
	const [verificationError, setVerificationError] = useState("");
	const [verifying, setVerifying] = useState(false);
	const generation = useRef(0);
	const jobGeneration = useRef(0);
	const reloadAccount = useCallback(async () => {
		const current = await invoke<GithubAccount>("github_auth_status");
		setAccount(current);
		return current;
	}, []);
	const loadRepositories = useCallback(async () => {
		setAuthBusy(true);
		setAuthMessage("");
		try {
			const repos = await invoke<RepoChoice[]>("github_repositories");
			setRepoChoices(repos);
		} catch (e) {
			setAuthMessage(String(e));
		} finally {
			setAuthBusy(false);
		}
	}, []);
	useEffect(() => {
		void reloadAccount().catch(() => {});
		void invoke<Record<string, string>>("load_settings")
			.then((s) => {
				const v = s["bloom-github-repository"];
				if (v) {
					setInput(v);
					setRepository(v);
				}
			})
			.catch(() => {});
	}, [reloadAccount]);
	useEffect(() => subscribeGithubDeviceFlow(setDeviceFlow), []);
	useEffect(() => {
		if (!deviceFlow.completedAt) return;
		setAuthMessage(`Sesión iniciada como ${deviceFlow.login}.`);
		void reloadAccount();
		void loadRepositories();
	}, [deviceFlow.completedAt, deviceFlow.login, loadRepositories, reloadAccount]);
	const beginLogin = async () => {
		setAuthBusy(true);
		setAuthMessage("");
		await startGithubDeviceFlow();
		setAuthBusy(false);
	};
	const copyCode = async () => {
		if (!deviceFlow.userCode) return;
		try {
			await writeText(deviceFlow.userCode);
			setCodeCopied(true);
			window.setTimeout(() => setCodeCopied(false), 1800);
		} catch {
			setAuthMessage("No se pudo copiar el código. Puedes seleccionarlo y copiarlo manualmente.");
		}
	};
	const saveRepositories = async () => {
		const selectedRepos = repoChoices.filter((repo) => repo.selected);
		setAuthBusy(true);
		try {
			await invoke("github_select_repositories", { repositories: selectedRepos });
			await reloadAccount();
			setAuthMessage(`${selectedRepos.length} repositorio(s) guardado(s) en este dispositivo.`);
		} catch (e) {
			setAuthMessage(String(e));
		} finally {
			setAuthBusy(false);
		}
	};
	const useAccountRepository = async (repository: RepoChoice) => {
		const selection = repoChoices.filter((item) => item.selected || item.id === repository.id);
		setAuthBusy(true);
		try {
			await invoke("github_select_repositories", { repositories: selection });
			await invoke("save_setting", {
				key: "bloom-github-repository",
				value: repository.fullName
			});
			setInput(repository.fullName);
			setRepository(repository.fullName);
			await emit("github-repository-changed", repository.fullName);
			setEditing(false);
			setError("");
			await reloadAccount();
		} catch (reason) {
			setAuthMessage(String(reason));
		} finally {
			setAuthBusy(false);
		}
	};
	const disconnect = async () => {
		setAuthBusy(true);
		try {
			await invoke("github_disconnect");
			setAccount({ connected: false, login: null, selected: [] });
			setRepoChoices([]);
			setAuthMessage("La cuenta y sus tokens protegidos se quitaron de este equipo.");
		} catch (e) {
			setAuthMessage(String(e));
		} finally {
			setAuthBusy(false);
		}
	};
	const verifyRelease = async () => {
		setVerifying(true);
		setVerification(null);
		setVerificationError("");
		try {
			setVerification(await invoke<ReleaseVerification>("github_release_verification"));
		} catch (e) {
			setVerificationError(String(e));
		} finally {
			setVerifying(false);
		}
	};
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
			await emit("github-repository-changed", value);
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
						<div className="github-auth-card">
							<div className="github-auth-heading">
								<strong>
									{account.connected ? `Conectado como ${account.login}` : "Cuenta GitHub"}
								</strong>
								{account.connected ? (
									<button
										disabled={authBusy}
										onClick={() => void disconnect()}
										title="Desconectar GitHub"
									>
										<LogOut size={15} /> Desconectar
									</button>
								) : (
									<button
										disabled={authBusy || deviceFlow.pending}
										onClick={() => void beginLogin()}
									>
										<LogIn size={15} />{" "}
										{deviceFlow.pending ? "Esperando a GitHub" : "Iniciar sesión"}
									</button>
								)}
							</div>
							{deviceFlow.userCode && (
								<div className="github-device-code-row">
									<p className="github-device-code">
										Código de autorización <strong>{deviceFlow.userCode}</strong>
									</p>
									<button className="github-copy-code" onClick={() => void copyCode()}>
										{codeCopied ? <CheckCheck size={15} /> : <Copy size={15} />}
										{codeCopied ? "Copiado" : "Copiar código"}
									</button>
								</div>
							)}
							{deviceFlow.message && (
								<p className="github-auth-message" role="status">
									{deviceFlow.message}
								</p>
							)}
							{deviceFlow.error && (
								<p
									className={deviceFlow.pending ? "github-auth-message" : "github-error"}
									role="status"
								>
									{deviceFlow.error}
								</p>
							)}
							{authMessage && (
								<p className="github-auth-message" role="status">
									{authMessage}
								</p>
							)}
							{account.connected && (
								<>
									<div className="github-repo-list-heading">
										<strong>Repositorios de tu cuenta · {account.login}</strong>
										<button disabled={authBusy} onClick={() => void loadRepositories()}>
											<RefreshCw size={14} /> Cargar
										</button>
									</div>
									{!authBusy && repoChoices.length === 0 && (
										<button
											className="github-install-app"
											onClick={() =>
												void openUrl("https://github.com/apps/bloom-florecer/installations/new")
											}
										>
											Autorizar instalación de Bloom en Asius <ExternalLink size={14} />
										</button>
									)}
									{repoChoices.length > 0 && (
										<div className="github-repo-choices">
											{[
												{
													label: "Públicos de tu cuenta",
													repos: repoChoices.filter((repo) => !repo.private)
												},
												{
													label: "Privados de tu cuenta",
													repos: repoChoices.filter((repo) => repo.private)
												}
											].map(
												({ label, repos }) =>
													repos.length > 0 && (
														<section className="github-repo-group" key={label}>
															<h4>{label}</h4>
															{repos.map((repo) => (
																<div className="github-repo-choice" key={repo.id}>
																	<label>
																		<input
																			type="checkbox"
																			checked={repo.selected}
																			onChange={(event) =>
																				setRepoChoices((items) =>
																					items.map((item) =>
																						item.id === repo.id
																							? { ...item, selected: event.target.checked }
																							: item
																					)
																				)
																			}
																		/>
																		<span>{repo.fullName}</span>
																	</label>
																	<button
																		disabled={authBusy}
																		title="Mostrar en el panel"
																		onClick={() => void useAccountRepository(repo)}
																	>
																		Usar
																	</button>
																</div>
															))}
														</section>
													)
											)}
										</div>
									)}
									{repoChoices.length > 0 && (
										<button
											className="github-save-repos"
											disabled={authBusy}
											onClick={() => void saveRepositories()}
										>
											Guardar repositorios seguidos
										</button>
									)}
								</>
							)}
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
					{verification?.verified ? <ShieldCheck size={21} /> : <Circle size={21} />}
					<strong>Canal y firma</strong>
					<span>
						{verification?.verified ? `Verificada · ${verification.version}` : "Sin comprobar"}
					</span>
					<button
						disabled={verifying}
						onClick={() => void verifyRelease()}
						title="Verificar firma del instalador latest"
					>
						{verifying ? <LoaderCircle className="github-spin" size={16} /> : "Comprobar"}
					</button>
				</div>
				{verification?.verified && (
					<p className="github-foot">
						{verification.detail} ({Math.ceil(verification.bytes / 1_048_576)} MB).
					</p>
				)}
				{verificationError && (
					<p className="github-error" role="alert">
						{verificationError}
					</p>
				)}
				{names.length > 3 && (
					<button className="github-other" onClick={() => setShowOther((v) => !v)}>
						{showOther ? "Ocultar otras ejecuciones" : "Otras ejecuciones"}
					</button>
				)}
				<p className="github-foot">
					{account.connected ? `Consulta autenticada · ${account.login}` : "Consulta pública"}
					{updated && ` · Actualizado ${new Date(updated).toLocaleTimeString()}`}
				</p>
			</div>
		</section>
	);
}
