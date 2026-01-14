(function (fa, require$$0, _manifest) {
    'use strict';

    const bgStyle1 = 'background: #16a085; color: black;';
    const log = (name, ...args) => {
        console.log(`%c @decky/ui %c ${name} %c`, bgStyle1, 'background: #1abc9c; color: black;', 'background: transparent;', ...args);
    };
    const group = (name, ...args) => {
        console.group(`%c @decky/ui %c ${name} %c`, bgStyle1, 'background: #1abc9c; color: black;', 'background: transparent;', ...args);
    };
    const groupEnd = (name, ...args) => {
        console.groupEnd();
        if (args?.length > 0)
            console.log(`^ %c @decky/ui %c ${name} %c`, bgStyle1, 'background: #1abc9c; color: black;', 'background: transparent;', ...args);
    };
    const debug = (name, ...args) => {
        console.debug(`%c @decky/ui %c ${name} %c`, bgStyle1, 'background: #1abc9c; color: black;', 'color: blue;', ...args);
    };
    const warn = (name, ...args) => {
        console.warn(`%c @decky/ui %c ${name} %c`, bgStyle1, 'background: #ffbb00; color: black;', 'color: blue;', ...args);
    };
    const error = (name, ...args) => {
        console.error(`%c @decky/ui %c ${name} %c`, bgStyle1, 'background: #FF0000;', 'background: transparent;', ...args);
    };
    class Logger {
        constructor(name) {
            this.name = name;
            this.name = name;
        }
        log(...args) {
            log(this.name, ...args);
        }
        debug(...args) {
            debug(this.name, ...args);
        }
        warn(...args) {
            warn(this.name, ...args);
        }
        error(...args) {
            error(this.name, ...args);
        }
        group(...args) {
            group(this.name, ...args);
        }
        groupEnd(...args) {
            groupEnd(this.name, ...args);
        }
    }

    const logger = new Logger('Webpack');
    let modules = new Map();
    function initModuleCache() {
        const startTime = performance.now();
        logger.group('Webpack Module Init');
        const id = Symbol("@decky/ui");
        let webpackRequire;
        window.webpackChunksteamui.push([
            [id],
            {},
            (r) => {
                webpackRequire = r;
            },
        ]);
        logger.log('Initializing all modules. Errors here likely do not matter, as they are usually just failing module side effects.');
        for (let id of Object.keys(webpackRequire.m)) {
            try {
                const module = webpackRequire(id);
                if (module) {
                    modules.set(id, module);
                }
            }
            catch (e) {
                logger.debug('Ignoring require error for module', id, e);
            }
        }
        logger.groupEnd(`Modules initialized in ${performance.now() - startTime}ms...`);
    }
    initModuleCache();
    const findModule = (filter) => {
        for (const m of modules.values()) {
            if (m.default && filter(m.default))
                return m.default;
            if (filter(m))
                return m;
        }
    };
    const findModuleDetailsByExport = (filter, minExports) => {
        for (const [id, m] of modules) {
            if (!m)
                continue;
            for (const mod of [m.default, m]) {
                if (typeof mod !== 'object')
                    continue;
                if (mod == window)
                    continue;
                for (let exportName in mod) {
                    if (mod?.[exportName]) {
                        try {
                            const filterRes = filter(mod[exportName], exportName);
                            if (filterRes) {
                                return [mod, mod[exportName], exportName, id];
                            }
                            else {
                                continue;
                            }
                        }
                        catch (e) {
                            logger.warn("Webpack filter threw exception: ", e);
                        }
                    }
                }
            }
        }
        return [undefined, undefined, undefined, undefined];
    };
    const findModuleByExport = (filter, minExports) => {
        return findModuleDetailsByExport(filter)?.[0];
    };
    const findModuleExport = (filter, minExports) => {
        return findModuleDetailsByExport(filter)?.[1];
    };
    const createModuleMapping = (filter) => {
        const mapping = new Map();
        for (const [id, m] of modules) {
            if (m.default && filter(m.default))
                mapping.set(id, m.default);
            if (filter(m))
                mapping.set(id, m);
        }
        return mapping;
    };
    const CommonUIModule = findModule((m) => {
        if (typeof m !== 'object')
            return false;
        for (let prop in m) {
            if (m[prop]?.contextType?._currentValue && Object.keys(m).length > 60)
                return true;
        }
        return false;
    });
    findModuleByExport((e) => e?.toString && /Spinner\)}\)?,.\.createElement\(\"path\",{d:\"M18 /.test(e.toString()));
    findModuleByExport((e) => e.computeRootMatch);

    const classModuleMap = createModuleMapping((m) => {
        if (typeof m == 'object' && !m.__esModule) {
            const keys = Object.keys(m);
            if (keys.length == 1 && m.version)
                return false;
            if (keys.length > 1000 && m.AboutSettings)
                return false;
            return keys.length > 0 && keys.every((k) => !Object.getOwnPropertyDescriptor(m, k)?.get && typeof m[k] == 'string');
        }
        return false;
    });
    const classMap = [...classModuleMap.values()];
    function findClassModule(filter) {
        return classMap.find((m) => filter(m));
    }

    const quickAccessMenuClasses = findClassModule((m) => m.Title && m.QuickAccessMenu && m.BatteryDetailsLabels);
    findClassModule((m) => m.ScrollPanel);
    findClassModule((m) => m.GamepadDialogContent && !m.BindingButtons);
    findClassModule((m) => m.BatteryPercentageLabel && m.PanelSection && !m['vr-dashboard-bar-height'] && !m.QuickAccessMenu && !m.QuickAccess && !m.PerfProfileInfo);
    findClassModule((m) => m.OOBEUpdateStatusContainer);
    findClassModule((m) => m.PlayBarDetailLabel);
    findClassModule((m) => m.SliderControlPanelGroup);
    findClassModule((m) => m.TopCapsule);
    findClassModule((m) => m.HeaderLoaded);
    findClassModule((m) => m.BasicUiRoot);
    findClassModule((m) => m.GamepadTabbedPage);
    findClassModule((m) => m.BasicContextMenuModal);
    findClassModule((m) => m.AchievementListItemBase && !m.Page);
    findClassModule((m) => m.AchievementListItemBase && m.Page);
    findClassModule((m) => m.AppRunningControls && m.OverlayAchievements);
    findClassModule((m) => m.AppDetailsRoot);
    findClassModule(m => m.SpinnerLoaderContainer);
    findClassModule(m => m.QuickAccessFooter);
    findClassModule(m => m.PlayButtonContainer);
    findClassModule(m => m.LongTitles && m.GreyBackground);
    findClassModule(m => m.GamepadLibrary);
    findClassModule(m => m.FocusRingRoot);
    findClassModule(m => m.SearchAndTitleContainer);
    findClassModule(m => m.MainBrowserContainer);
    const staticClasses = quickAccessMenuClasses;

    const Field = findModuleExport((e) => e?.render?.toString().includes('"shift-children-below"'));

    const [mod, panelSection] = findModuleDetailsByExport((e) => e.toString()?.includes('.PanelSection'));
    const PanelSection = panelSection;
    const PanelSectionRow = Object.values(mod).filter((exp) => !exp?.toString?.()?.includes('.PanelSection'))[0];

    const SliderField = Object.values(CommonUIModule).find((mod) => mod?.toString?.()?.includes('SliderField,fallback') || mod?.toString?.()?.includes("SliderField\","));

    const ToggleField = Object.values(CommonUIModule).find((mod) => mod?.render?.toString?.()?.includes('ToggleField,fallback') || mod?.render?.toString?.()?.includes("ToggleField\","));

    const definePlugin = (fn) => {
        return (...args) => {
            return fn(...args);
        };
    };

    const manifest = _manifest;
    const API_VERSION = 2;
    if (!manifest?.name) {
        throw new Error('[@decky/api]: Failed to find plugin manifest.');
    }
    const internalAPIConnection = window.__DECKY_SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED_deckyLoaderAPIInit;
    if (!internalAPIConnection) {
        throw new Error('[@decky/api]: Failed to connect to the loader as as the loader API was not initialized. This is likely a bug in Decky Loader.');
    }
    let api;
    try {
        api = internalAPIConnection.connect(API_VERSION, manifest.name);
    }
    catch {
        api = internalAPIConnection.connect(1, manifest.name);
        console.warn(`[@decky/api] Requested API version ${API_VERSION} but the running loader only supports version 1. Some features may not work.`);
    }
    if (api._version != API_VERSION) {
        console.warn(`[@decky/api] Requested API version ${API_VERSION} but the running loader only supports version ${api._version}. Some features may not work.`);
    }
    const call = api.call;
    api.callable;
    api.addEventListener;
    api.removeEventListener;
    api.routerHook;
    api.toaster;
    api.openFilePicker;
    api.executeInTab;
    api.injectCssIntoTab;
    api.removeCssFromTab;
    api.fetchNoCors;
    api.getExternalResourceURL;
    api.useQuickAccessVisible;

    async function getStatus() {
        try {
            const result = await call("get_status");
            return result;
        }
        catch (error) {
            console.error("SmartRefresh: Failed to get status", error);
            return null;
        }
    }
    async function startDaemon() {
        try {
            await call("start_daemon");
            return true;
        }
        catch (error) {
            console.error("SmartRefresh: Failed to start daemon", error);
            return false;
        }
    }
    async function stopDaemon() {
        try {
            await call("stop_daemon");
            return true;
        }
        catch (error) {
            console.error("SmartRefresh: Failed to stop daemon", error);
            return false;
        }
    }
    async function setSettings(minHz, maxHz, sensitivity) {
        try {
            await call("set_settings", minHz, maxHz, sensitivity);
            return true;
        }
        catch (error) {
            console.error("SmartRefresh: Failed to set settings", error);
            return false;
        }
    }

    function EnableToggle() {
        const [enabled, setEnabled] = require$$0.useState(false);
        const [loading, setLoading] = require$$0.useState(true);
        require$$0.useEffect(() => {
            const fetchStatus = async () => {
                const status = await getStatus();
                if (status) {
                    setEnabled(status.running);
                }
                setLoading(false);
            };
            fetchStatus();
        }, []);
        const handleToggle = async (value) => {
            setLoading(true);
            const success = value ? await startDaemon() : await stopDaemon();
            if (success) {
                setEnabled(value);
            }
            setLoading(false);
        };
        return (React.createElement(ToggleField, { label: "Enable SmartRefresh", description: "Toggle dynamic refresh rate control", checked: enabled, disabled: loading, onChange: handleToggle }));
    }

    function RefreshRangeSlider() {
        const [minHz, setMinHz] = require$$0.useState(40);
        const [maxHz, setMaxHz] = require$$0.useState(90);
        const [sensitivity, setSensitivity] = require$$0.useState("balanced");
        const [loading, setLoading] = require$$0.useState(true);
        require$$0.useEffect(() => {
            const fetchStatus = async () => {
                const status = await getStatus();
                if (status) {
                    setMinHz(status.config.min_hz);
                    setMaxHz(status.config.max_hz);
                    setSensitivity(status.config.sensitivity);
                }
                setLoading(false);
            };
            fetchStatus();
        }, []);
        const handleMinChange = async (value) => {
            const newMin = Math.min(value, maxHz);
            setMinHz(newMin);
            await setSettings(newMin, maxHz, sensitivity);
        };
        const handleMaxChange = async (value) => {
            const newMax = Math.max(value, minHz);
            setMaxHz(newMax);
            await setSettings(minHz, newMax, sensitivity);
        };
        return (React.createElement("div", null,
            React.createElement(SliderField, { label: "Minimum Hz", description: `${minHz} Hz`, value: minHz, min: 40, max: 90, step: 5, disabled: loading, onChange: handleMinChange }),
            React.createElement(SliderField, { label: "Maximum Hz", description: `${maxHz} Hz`, value: maxHz, min: 40, max: 90, step: 5, disabled: loading, onChange: handleMaxChange })));
    }

    const SENSITIVITY_OPTIONS = ["conservative", "balanced", "aggressive"];
    const SENSITIVITY_LABELS = {
        conservative: "Conservative (slower transitions)",
        balanced: "Balanced (default)",
        aggressive: "Aggressive (faster transitions)",
    };
    function SensitivitySlider() {
        const [sensitivityIndex, setSensitivityIndex] = require$$0.useState(1);
        const [minHz, setMinHz] = require$$0.useState(40);
        const [maxHz, setMaxHz] = require$$0.useState(90);
        const [loading, setLoading] = require$$0.useState(true);
        require$$0.useEffect(() => {
            const fetchStatus = async () => {
                const status = await getStatus();
                if (status) {
                    const index = SENSITIVITY_OPTIONS.indexOf(status.config.sensitivity);
                    setSensitivityIndex(index >= 0 ? index : 1);
                    setMinHz(status.config.min_hz);
                    setMaxHz(status.config.max_hz);
                }
                setLoading(false);
            };
            fetchStatus();
        }, []);
        const handleChange = async (value) => {
            setSensitivityIndex(value);
            const sensitivity = SENSITIVITY_OPTIONS[value];
            await setSettings(minHz, maxHz, sensitivity);
        };
        const currentSensitivity = SENSITIVITY_OPTIONS[sensitivityIndex];
        return (React.createElement(SliderField, { label: "Sensitivity", description: SENSITIVITY_LABELS[currentSensitivity], value: sensitivityIndex, min: 0, max: 2, step: 1, notchCount: 3, notchLabels: [
                { notchIndex: 0, label: "Conservative" },
                { notchIndex: 1, label: "Balanced" },
                { notchIndex: 2, label: "Aggressive" },
            ], disabled: loading, onChange: handleChange }));
    }

    function DebugView() {
        const [status, setStatus] = require$$0.useState(null);
        const [error, setError] = require$$0.useState(false);
        const intervalRef = require$$0.useRef(null);
        require$$0.useEffect(() => {
            const fetchStatus = async () => {
                const result = await getStatus();
                if (result) {
                    setStatus(result);
                    setError(false);
                }
                else {
                    setError(true);
                }
            };
            // Initial fetch
            fetchStatus();
            // Poll every 500ms while panel is open
            intervalRef.current = window.setInterval(fetchStatus, 500);
            return () => {
                if (intervalRef.current) {
                    window.clearInterval(intervalRef.current);
                }
            };
        }, []);
        if (error) {
            return (React.createElement(Field, { label: "Status" },
                React.createElement("div", { style: { color: "#ff6b6b" } }, "\u26A0\uFE0F Daemon unreachable")));
        }
        if (!status) {
            return (React.createElement(Field, { label: "Status" },
                React.createElement("div", null, "Loading...")));
        }
        return (React.createElement("div", null,
            React.createElement(Field, { label: "Current FPS" },
                React.createElement("div", null, status.current_fps.toFixed(1))),
            React.createElement(Field, { label: "Current Hz" },
                React.createElement("div", null,
                    status.current_hz,
                    " Hz")),
            React.createElement(Field, { label: "State" },
                React.createElement("div", null, status.state))));
    }

    var index = definePlugin(() => {
        return {
            name: "SmartRefresh",
            title: React.createElement("div", { className: staticClasses.Title }, "SmartRefresh"),
            content: (React.createElement(PanelSection, { title: "SmartRefresh" },
                React.createElement(PanelSectionRow, null,
                    React.createElement(EnableToggle, null)),
                React.createElement(PanelSectionRow, null,
                    React.createElement(RefreshRangeSlider, null)),
                React.createElement(PanelSectionRow, null,
                    React.createElement(SensitivitySlider, null)),
                React.createElement(PanelSectionRow, null,
                    React.createElement(DebugView, null)))),
            icon: React.createElement(fa.FaSync, null),
        };
    });

    return index;

})(fa, SP_REACT, _manifest);
