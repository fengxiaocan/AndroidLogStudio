import type { DeviceInfo } from '../types/protocol';

interface DeviceTabsProps {
  devices: DeviceInfo[];
  activeDeviceId: string | null;
  onSelect?: (deviceId: string) => void;
}

export function DeviceTabs({ devices, activeDeviceId, onSelect }: DeviceTabsProps) {
  return (
    <nav className="device-tabs" aria-label="Connected devices">
      {devices.length === 0 ? (
        <span className="device-tabs__empty">No devices connected</span>
      ) : (
        devices.map((device) => {
          const active = device.deviceId === activeDeviceId;
          const className = [
            'device-tab',
            active ? 'active' : '',
            device.connected ? '' : 'device-tab--disconnected',
          ]
            .filter(Boolean)
            .join(' ');

          return (
            <button
              className={className}
              key={device.deviceId}
              type="button"
              aria-current={active ? 'page' : undefined}
              onClick={() => onSelect?.(device.deviceId)}
            >
              <span className="device-tab__name">{device.deviceName}</span>
              <span className="device-tab__id">{device.deviceId}</span>
              <span className="device-tab__source">
                {device.connected ? `Source: ${device.source.toUpperCase()}` : 'Disconnected'}
              </span>
            </button>
          );
        })
      )}
    </nav>
  );
}
