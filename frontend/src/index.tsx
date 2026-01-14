import {
  definePlugin,
  PanelSection,
  PanelSectionRow,
  staticClasses,
} from "@decky/ui";
import { FaSync } from "react-icons/fa";
import { EnableToggle } from "./components/EnableToggle";
import { RefreshRangeSlider } from "./components/RefreshRangeSlider";
import { SensitivitySlider } from "./components/SensitivitySlider";
import { DebugView } from "./components/DebugView";

export default definePlugin(() => {
  return {
    name: "SmartRefresh",
    title: <div className={staticClasses.Title}>SmartRefresh</div>,
    content: (
      <PanelSection title="SmartRefresh">
        <PanelSectionRow>
          <EnableToggle />
        </PanelSectionRow>
        <PanelSectionRow>
          <RefreshRangeSlider />
        </PanelSectionRow>
        <PanelSectionRow>
          <SensitivitySlider />
        </PanelSectionRow>
        <PanelSectionRow>
          <DebugView />
        </PanelSectionRow>
      </PanelSection>
    ),
    icon: <FaSync />,
  };
});
