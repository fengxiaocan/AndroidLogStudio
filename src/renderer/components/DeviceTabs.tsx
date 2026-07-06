import type { DeviceInfo } from '../types/protocol';

interface DeviceTabsProps {
  devices: DeviceInfo[];
  activeDeviceId: string | null;
}

export function DeviceTabs({ devices, activeDeviceId }: DeviceTabsProps) {
  return (
    <nav className="device-tabs" aria-label="Connected devices">
      {devices.length === 0 ? (
        <span className="device-tabs__empty">No devices connected</span>
      ) : (
        devices.map((device) => {
          const active = device.deviceId === activeDeviceId;
          return (
            <button
              className={active ? 'device-tab active' : 'device-tab'}
              key={device.deviceId}
              type="button"
              aria-current={active ? 'page' : undefined}
            >
              <span className="device-tab__name">{device.deviceName}</span>
              <span className="device-tab__id">{device.deviceId}</span>
              <span className="device-tab__source">Source: {device.source.toUpperCase()}</span>
            </button>
          );
        })
      )}
    </nav>
  );
}
