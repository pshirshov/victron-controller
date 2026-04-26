// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {ActualF64, ActualF64_UEBACodec} from './ActualF64'

export class Sensors implements BaboonGeneratedLatest {
    private readonly _battery_soc: ActualF64;
    private readonly _battery_soh: ActualF64;
    private readonly _battery_installed_capacity: ActualF64;
    private readonly _battery_dc_power: ActualF64;
    private readonly _mppt_power_0: ActualF64;
    private readonly _mppt_power_1: ActualF64;
    private readonly _soltaro_power: ActualF64;
    private readonly _power_consumption: ActualF64;
    private readonly _grid_power: ActualF64;
    private readonly _grid_voltage: ActualF64;
    private readonly _grid_current: ActualF64;
    private readonly _consumption_current: ActualF64;
    private readonly _offgrid_power: ActualF64;
    private readonly _offgrid_current: ActualF64;
    private readonly _vebus_input_current: ActualF64;
    private readonly _evcharger_ac_power: ActualF64;
    private readonly _evcharger_ac_current: ActualF64;
    private readonly _ess_state: ActualF64;
    private readonly _outdoor_temperature: ActualF64;
    private readonly _session_kwh: ActualF64;
    private readonly _ev_soc: ActualF64;
    private readonly _ev_charge_target: ActualF64;

    constructor(battery_soc: ActualF64, battery_soh: ActualF64, battery_installed_capacity: ActualF64, battery_dc_power: ActualF64, mppt_power_0: ActualF64, mppt_power_1: ActualF64, soltaro_power: ActualF64, power_consumption: ActualF64, grid_power: ActualF64, grid_voltage: ActualF64, grid_current: ActualF64, consumption_current: ActualF64, offgrid_power: ActualF64, offgrid_current: ActualF64, vebus_input_current: ActualF64, evcharger_ac_power: ActualF64, evcharger_ac_current: ActualF64, ess_state: ActualF64, outdoor_temperature: ActualF64, session_kwh: ActualF64, ev_soc: ActualF64, ev_charge_target: ActualF64) {
        this._battery_soc = battery_soc
        this._battery_soh = battery_soh
        this._battery_installed_capacity = battery_installed_capacity
        this._battery_dc_power = battery_dc_power
        this._mppt_power_0 = mppt_power_0
        this._mppt_power_1 = mppt_power_1
        this._soltaro_power = soltaro_power
        this._power_consumption = power_consumption
        this._grid_power = grid_power
        this._grid_voltage = grid_voltage
        this._grid_current = grid_current
        this._consumption_current = consumption_current
        this._offgrid_power = offgrid_power
        this._offgrid_current = offgrid_current
        this._vebus_input_current = vebus_input_current
        this._evcharger_ac_power = evcharger_ac_power
        this._evcharger_ac_current = evcharger_ac_current
        this._ess_state = ess_state
        this._outdoor_temperature = outdoor_temperature
        this._session_kwh = session_kwh
        this._ev_soc = ev_soc
        this._ev_charge_target = ev_charge_target
    }

    public get battery_soc(): ActualF64 {
        return this._battery_soc;
    }
    public get battery_soh(): ActualF64 {
        return this._battery_soh;
    }
    public get battery_installed_capacity(): ActualF64 {
        return this._battery_installed_capacity;
    }
    public get battery_dc_power(): ActualF64 {
        return this._battery_dc_power;
    }
    public get mppt_power_0(): ActualF64 {
        return this._mppt_power_0;
    }
    public get mppt_power_1(): ActualF64 {
        return this._mppt_power_1;
    }
    public get soltaro_power(): ActualF64 {
        return this._soltaro_power;
    }
    public get power_consumption(): ActualF64 {
        return this._power_consumption;
    }
    public get grid_power(): ActualF64 {
        return this._grid_power;
    }
    public get grid_voltage(): ActualF64 {
        return this._grid_voltage;
    }
    public get grid_current(): ActualF64 {
        return this._grid_current;
    }
    public get consumption_current(): ActualF64 {
        return this._consumption_current;
    }
    public get offgrid_power(): ActualF64 {
        return this._offgrid_power;
    }
    public get offgrid_current(): ActualF64 {
        return this._offgrid_current;
    }
    public get vebus_input_current(): ActualF64 {
        return this._vebus_input_current;
    }
    public get evcharger_ac_power(): ActualF64 {
        return this._evcharger_ac_power;
    }
    public get evcharger_ac_current(): ActualF64 {
        return this._evcharger_ac_current;
    }
    public get ess_state(): ActualF64 {
        return this._ess_state;
    }
    public get outdoor_temperature(): ActualF64 {
        return this._outdoor_temperature;
    }
    public get session_kwh(): ActualF64 {
        return this._session_kwh;
    }
    public get ev_soc(): ActualF64 {
        return this._ev_soc;
    }
    public get ev_charge_target(): ActualF64 {
        return this._ev_charge_target;
    }

    public toJSON(): Record<string, unknown> {
        return {
            battery_soc: this._battery_soc,
            battery_soh: this._battery_soh,
            battery_installed_capacity: this._battery_installed_capacity,
            battery_dc_power: this._battery_dc_power,
            mppt_power_0: this._mppt_power_0,
            mppt_power_1: this._mppt_power_1,
            soltaro_power: this._soltaro_power,
            power_consumption: this._power_consumption,
            grid_power: this._grid_power,
            grid_voltage: this._grid_voltage,
            grid_current: this._grid_current,
            consumption_current: this._consumption_current,
            offgrid_power: this._offgrid_power,
            offgrid_current: this._offgrid_current,
            vebus_input_current: this._vebus_input_current,
            evcharger_ac_power: this._evcharger_ac_power,
            evcharger_ac_current: this._evcharger_ac_current,
            ess_state: this._ess_state,
            outdoor_temperature: this._outdoor_temperature,
            session_kwh: this._session_kwh,
            ev_soc: this._ev_soc,
            ev_charge_target: this._ev_charge_target
        };
    }

    public with(overrides: {battery_soc?: ActualF64; battery_soh?: ActualF64; battery_installed_capacity?: ActualF64; battery_dc_power?: ActualF64; mppt_power_0?: ActualF64; mppt_power_1?: ActualF64; soltaro_power?: ActualF64; power_consumption?: ActualF64; grid_power?: ActualF64; grid_voltage?: ActualF64; grid_current?: ActualF64; consumption_current?: ActualF64; offgrid_power?: ActualF64; offgrid_current?: ActualF64; vebus_input_current?: ActualF64; evcharger_ac_power?: ActualF64; evcharger_ac_current?: ActualF64; ess_state?: ActualF64; outdoor_temperature?: ActualF64; session_kwh?: ActualF64; ev_soc?: ActualF64; ev_charge_target?: ActualF64}): Sensors {
        return new Sensors(
            'battery_soc' in overrides ? overrides.battery_soc! : this._battery_soc,
            'battery_soh' in overrides ? overrides.battery_soh! : this._battery_soh,
            'battery_installed_capacity' in overrides ? overrides.battery_installed_capacity! : this._battery_installed_capacity,
            'battery_dc_power' in overrides ? overrides.battery_dc_power! : this._battery_dc_power,
            'mppt_power_0' in overrides ? overrides.mppt_power_0! : this._mppt_power_0,
            'mppt_power_1' in overrides ? overrides.mppt_power_1! : this._mppt_power_1,
            'soltaro_power' in overrides ? overrides.soltaro_power! : this._soltaro_power,
            'power_consumption' in overrides ? overrides.power_consumption! : this._power_consumption,
            'grid_power' in overrides ? overrides.grid_power! : this._grid_power,
            'grid_voltage' in overrides ? overrides.grid_voltage! : this._grid_voltage,
            'grid_current' in overrides ? overrides.grid_current! : this._grid_current,
            'consumption_current' in overrides ? overrides.consumption_current! : this._consumption_current,
            'offgrid_power' in overrides ? overrides.offgrid_power! : this._offgrid_power,
            'offgrid_current' in overrides ? overrides.offgrid_current! : this._offgrid_current,
            'vebus_input_current' in overrides ? overrides.vebus_input_current! : this._vebus_input_current,
            'evcharger_ac_power' in overrides ? overrides.evcharger_ac_power! : this._evcharger_ac_power,
            'evcharger_ac_current' in overrides ? overrides.evcharger_ac_current! : this._evcharger_ac_current,
            'ess_state' in overrides ? overrides.ess_state! : this._ess_state,
            'outdoor_temperature' in overrides ? overrides.outdoor_temperature! : this._outdoor_temperature,
            'session_kwh' in overrides ? overrides.session_kwh! : this._session_kwh,
            'ev_soc' in overrides ? overrides.ev_soc! : this._ev_soc,
            'ev_charge_target' in overrides ? overrides.ev_charge_target! : this._ev_charge_target
        );
    }

    public static fromPlain(obj: {battery_soc: ActualF64; battery_soh: ActualF64; battery_installed_capacity: ActualF64; battery_dc_power: ActualF64; mppt_power_0: ActualF64; mppt_power_1: ActualF64; soltaro_power: ActualF64; power_consumption: ActualF64; grid_power: ActualF64; grid_voltage: ActualF64; grid_current: ActualF64; consumption_current: ActualF64; offgrid_power: ActualF64; offgrid_current: ActualF64; vebus_input_current: ActualF64; evcharger_ac_power: ActualF64; evcharger_ac_current: ActualF64; ess_state: ActualF64; outdoor_temperature: ActualF64; session_kwh: ActualF64; ev_soc: ActualF64; ev_charge_target: ActualF64}): Sensors {
        return new Sensors(
            obj.battery_soc,
            obj.battery_soh,
            obj.battery_installed_capacity,
            obj.battery_dc_power,
            obj.mppt_power_0,
            obj.mppt_power_1,
            obj.soltaro_power,
            obj.power_consumption,
            obj.grid_power,
            obj.grid_voltage,
            obj.grid_current,
            obj.consumption_current,
            obj.offgrid_power,
            obj.offgrid_current,
            obj.vebus_input_current,
            obj.evcharger_ac_power,
            obj.evcharger_ac_current,
            obj.ess_state,
            obj.outdoor_temperature,
            obj.session_kwh,
            obj.ev_soc,
            obj.ev_charge_target
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Sensors.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Sensors.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Sensors'
    public baboonTypeIdentifier() {
        return Sensors.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0"]
    public baboonSameInVersions() {
        return Sensors.BaboonSameInVersions
    }
    public static binCodec(): Sensors_UEBACodec {
        return Sensors_UEBACodec.instance
    }
}

export class Sensors_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Sensors, writer: BaboonBinWriter): unknown {
        if (this !== Sensors_UEBACodec.lazyInstance.value) {
          return Sensors_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.battery_soc, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.battery_soh, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.battery_installed_capacity, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.battery_dc_power, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.mppt_power_0, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.mppt_power_1, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.soltaro_power, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.power_consumption, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.grid_power, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.grid_voltage, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.grid_current, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.consumption_current, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.offgrid_power, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.offgrid_current, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.vebus_input_current, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.evcharger_ac_power, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.evcharger_ac_current, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.ess_state, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.outdoor_temperature, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.session_kwh, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.ev_soc, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActualF64_UEBACodec.instance.encode(ctx, value.ev_charge_target, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            ActualF64_UEBACodec.instance.encode(ctx, value.battery_soc, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.battery_soh, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.battery_installed_capacity, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.battery_dc_power, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.mppt_power_0, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.mppt_power_1, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.soltaro_power, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.power_consumption, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.grid_power, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.grid_voltage, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.grid_current, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.consumption_current, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.offgrid_power, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.offgrid_current, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.vebus_input_current, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.evcharger_ac_power, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.evcharger_ac_current, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.ess_state, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.outdoor_temperature, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.session_kwh, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.ev_soc, writer);
            ActualF64_UEBACodec.instance.encode(ctx, value.ev_charge_target, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Sensors {
        if (this !== Sensors_UEBACodec .lazyInstance.value) {
            return Sensors_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 22; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const battery_soc = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const battery_soh = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const battery_installed_capacity = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const battery_dc_power = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const mppt_power_0 = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const mppt_power_1 = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const soltaro_power = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const power_consumption = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const grid_power = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const grid_voltage = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const grid_current = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const consumption_current = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const offgrid_power = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const offgrid_current = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const vebus_input_current = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const evcharger_ac_power = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const evcharger_ac_current = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const ess_state = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const outdoor_temperature = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const session_kwh = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const ev_soc = ActualF64_UEBACodec.instance.decode(ctx, reader);
        const ev_charge_target = ActualF64_UEBACodec.instance.decode(ctx, reader);
        return new Sensors(
            battery_soc,
            battery_soh,
            battery_installed_capacity,
            battery_dc_power,
            mppt_power_0,
            mppt_power_1,
            soltaro_power,
            power_consumption,
            grid_power,
            grid_voltage,
            grid_current,
            consumption_current,
            offgrid_power,
            offgrid_current,
            vebus_input_current,
            evcharger_ac_power,
            evcharger_ac_current,
            ess_state,
            outdoor_temperature,
            session_kwh,
            ev_soc,
            ev_charge_target,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Sensors_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Sensors_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Sensors'
    public baboonTypeIdentifier() {
        return Sensors_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Sensors_UEBACodec())
    public static get instance(): Sensors_UEBACodec {
        return Sensors_UEBACodec.lazyInstance.value
    }
}