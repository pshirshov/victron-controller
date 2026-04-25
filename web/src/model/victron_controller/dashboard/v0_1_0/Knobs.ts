// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {ChargeBatteryExtendedMode, ChargeBatteryExtendedMode_UEBACodec} from './ChargeBatteryExtendedMode'
import {DebugFullCharge, DebugFullCharge_UEBACodec} from './DebugFullCharge'
import {ForecastDisagreementStrategy, ForecastDisagreementStrategy_UEBACodec} from './ForecastDisagreementStrategy'
import {DischargeTime, DischargeTime_UEBACodec} from './DischargeTime'

export class Knobs implements BaboonGenerated {
    private readonly _force_disable_export: boolean;
    private readonly _export_soc_threshold: number;
    private readonly _discharge_soc_target: number;
    private readonly _battery_soc_target: number;
    private readonly _full_charge_discharge_soc_target: number;
    private readonly _full_charge_export_soc_threshold: number;
    private readonly _discharge_time: DischargeTime;
    private readonly _debug_full_charge: DebugFullCharge;
    private readonly _pessimism_multiplier_modifier: number;
    private readonly _disable_night_grid_discharge: boolean;
    private readonly _charge_car_boost: boolean;
    private readonly _charge_car_extended: boolean;
    private readonly _zappi_current_target: number;
    private readonly _zappi_limit: number;
    private readonly _zappi_emergency_margin: number;
    private readonly _grid_export_limit_w: number;
    private readonly _grid_import_limit_w: number;
    private readonly _allow_battery_to_car: boolean;
    private readonly _eddi_enable_soc: number;
    private readonly _eddi_disable_soc: number;
    private readonly _eddi_dwell_s: number;
    private readonly _weathersoc_winter_temperature_threshold: number;
    private readonly _weathersoc_low_energy_threshold: number;
    private readonly _weathersoc_ok_energy_threshold: number;
    private readonly _weathersoc_high_energy_threshold: number;
    private readonly _weathersoc_too_much_energy_threshold: number;
    private readonly _writes_enabled: boolean;
    private readonly _forecast_disagreement_strategy: ForecastDisagreementStrategy;
    private readonly _charge_battery_extended_mode: ChargeBatteryExtendedMode;

    constructor(force_disable_export: boolean, export_soc_threshold: number, discharge_soc_target: number, battery_soc_target: number, full_charge_discharge_soc_target: number, full_charge_export_soc_threshold: number, discharge_time: DischargeTime, debug_full_charge: DebugFullCharge, pessimism_multiplier_modifier: number, disable_night_grid_discharge: boolean, charge_car_boost: boolean, charge_car_extended: boolean, zappi_current_target: number, zappi_limit: number, zappi_emergency_margin: number, grid_export_limit_w: number, grid_import_limit_w: number, allow_battery_to_car: boolean, eddi_enable_soc: number, eddi_disable_soc: number, eddi_dwell_s: number, weathersoc_winter_temperature_threshold: number, weathersoc_low_energy_threshold: number, weathersoc_ok_energy_threshold: number, weathersoc_high_energy_threshold: number, weathersoc_too_much_energy_threshold: number, writes_enabled: boolean, forecast_disagreement_strategy: ForecastDisagreementStrategy, charge_battery_extended_mode: ChargeBatteryExtendedMode) {
        this._force_disable_export = force_disable_export
        this._export_soc_threshold = export_soc_threshold
        this._discharge_soc_target = discharge_soc_target
        this._battery_soc_target = battery_soc_target
        this._full_charge_discharge_soc_target = full_charge_discharge_soc_target
        this._full_charge_export_soc_threshold = full_charge_export_soc_threshold
        this._discharge_time = discharge_time
        this._debug_full_charge = debug_full_charge
        this._pessimism_multiplier_modifier = pessimism_multiplier_modifier
        this._disable_night_grid_discharge = disable_night_grid_discharge
        this._charge_car_boost = charge_car_boost
        this._charge_car_extended = charge_car_extended
        this._zappi_current_target = zappi_current_target
        this._zappi_limit = zappi_limit
        this._zappi_emergency_margin = zappi_emergency_margin
        this._grid_export_limit_w = grid_export_limit_w
        this._grid_import_limit_w = grid_import_limit_w
        this._allow_battery_to_car = allow_battery_to_car
        this._eddi_enable_soc = eddi_enable_soc
        this._eddi_disable_soc = eddi_disable_soc
        this._eddi_dwell_s = eddi_dwell_s
        this._weathersoc_winter_temperature_threshold = weathersoc_winter_temperature_threshold
        this._weathersoc_low_energy_threshold = weathersoc_low_energy_threshold
        this._weathersoc_ok_energy_threshold = weathersoc_ok_energy_threshold
        this._weathersoc_high_energy_threshold = weathersoc_high_energy_threshold
        this._weathersoc_too_much_energy_threshold = weathersoc_too_much_energy_threshold
        this._writes_enabled = writes_enabled
        this._forecast_disagreement_strategy = forecast_disagreement_strategy
        this._charge_battery_extended_mode = charge_battery_extended_mode
    }

    public get force_disable_export(): boolean {
        return this._force_disable_export;
    }
    public get export_soc_threshold(): number {
        return this._export_soc_threshold;
    }
    public get discharge_soc_target(): number {
        return this._discharge_soc_target;
    }
    public get battery_soc_target(): number {
        return this._battery_soc_target;
    }
    public get full_charge_discharge_soc_target(): number {
        return this._full_charge_discharge_soc_target;
    }
    public get full_charge_export_soc_threshold(): number {
        return this._full_charge_export_soc_threshold;
    }
    public get discharge_time(): DischargeTime {
        return this._discharge_time;
    }
    public get debug_full_charge(): DebugFullCharge {
        return this._debug_full_charge;
    }
    public get pessimism_multiplier_modifier(): number {
        return this._pessimism_multiplier_modifier;
    }
    public get disable_night_grid_discharge(): boolean {
        return this._disable_night_grid_discharge;
    }
    public get charge_car_boost(): boolean {
        return this._charge_car_boost;
    }
    public get charge_car_extended(): boolean {
        return this._charge_car_extended;
    }
    public get zappi_current_target(): number {
        return this._zappi_current_target;
    }
    public get zappi_limit(): number {
        return this._zappi_limit;
    }
    public get zappi_emergency_margin(): number {
        return this._zappi_emergency_margin;
    }
    public get grid_export_limit_w(): number {
        return this._grid_export_limit_w;
    }
    public get grid_import_limit_w(): number {
        return this._grid_import_limit_w;
    }
    public get allow_battery_to_car(): boolean {
        return this._allow_battery_to_car;
    }
    public get eddi_enable_soc(): number {
        return this._eddi_enable_soc;
    }
    public get eddi_disable_soc(): number {
        return this._eddi_disable_soc;
    }
    public get eddi_dwell_s(): number {
        return this._eddi_dwell_s;
    }
    public get weathersoc_winter_temperature_threshold(): number {
        return this._weathersoc_winter_temperature_threshold;
    }
    public get weathersoc_low_energy_threshold(): number {
        return this._weathersoc_low_energy_threshold;
    }
    public get weathersoc_ok_energy_threshold(): number {
        return this._weathersoc_ok_energy_threshold;
    }
    public get weathersoc_high_energy_threshold(): number {
        return this._weathersoc_high_energy_threshold;
    }
    public get weathersoc_too_much_energy_threshold(): number {
        return this._weathersoc_too_much_energy_threshold;
    }
    public get writes_enabled(): boolean {
        return this._writes_enabled;
    }
    public get forecast_disagreement_strategy(): ForecastDisagreementStrategy {
        return this._forecast_disagreement_strategy;
    }
    public get charge_battery_extended_mode(): ChargeBatteryExtendedMode {
        return this._charge_battery_extended_mode;
    }

    public toJSON(): Record<string, unknown> {
        return {
            force_disable_export: this._force_disable_export,
            export_soc_threshold: this._export_soc_threshold,
            discharge_soc_target: this._discharge_soc_target,
            battery_soc_target: this._battery_soc_target,
            full_charge_discharge_soc_target: this._full_charge_discharge_soc_target,
            full_charge_export_soc_threshold: this._full_charge_export_soc_threshold,
            discharge_time: this._discharge_time,
            debug_full_charge: this._debug_full_charge,
            pessimism_multiplier_modifier: this._pessimism_multiplier_modifier,
            disable_night_grid_discharge: this._disable_night_grid_discharge,
            charge_car_boost: this._charge_car_boost,
            charge_car_extended: this._charge_car_extended,
            zappi_current_target: this._zappi_current_target,
            zappi_limit: this._zappi_limit,
            zappi_emergency_margin: this._zappi_emergency_margin,
            grid_export_limit_w: this._grid_export_limit_w,
            grid_import_limit_w: this._grid_import_limit_w,
            allow_battery_to_car: this._allow_battery_to_car,
            eddi_enable_soc: this._eddi_enable_soc,
            eddi_disable_soc: this._eddi_disable_soc,
            eddi_dwell_s: this._eddi_dwell_s,
            weathersoc_winter_temperature_threshold: this._weathersoc_winter_temperature_threshold,
            weathersoc_low_energy_threshold: this._weathersoc_low_energy_threshold,
            weathersoc_ok_energy_threshold: this._weathersoc_ok_energy_threshold,
            weathersoc_high_energy_threshold: this._weathersoc_high_energy_threshold,
            weathersoc_too_much_energy_threshold: this._weathersoc_too_much_energy_threshold,
            writes_enabled: this._writes_enabled,
            forecast_disagreement_strategy: this._forecast_disagreement_strategy,
            charge_battery_extended_mode: this._charge_battery_extended_mode
        };
    }

    public with(overrides: {force_disable_export?: boolean; export_soc_threshold?: number; discharge_soc_target?: number; battery_soc_target?: number; full_charge_discharge_soc_target?: number; full_charge_export_soc_threshold?: number; discharge_time?: DischargeTime; debug_full_charge?: DebugFullCharge; pessimism_multiplier_modifier?: number; disable_night_grid_discharge?: boolean; charge_car_boost?: boolean; charge_car_extended?: boolean; zappi_current_target?: number; zappi_limit?: number; zappi_emergency_margin?: number; grid_export_limit_w?: number; grid_import_limit_w?: number; allow_battery_to_car?: boolean; eddi_enable_soc?: number; eddi_disable_soc?: number; eddi_dwell_s?: number; weathersoc_winter_temperature_threshold?: number; weathersoc_low_energy_threshold?: number; weathersoc_ok_energy_threshold?: number; weathersoc_high_energy_threshold?: number; weathersoc_too_much_energy_threshold?: number; writes_enabled?: boolean; forecast_disagreement_strategy?: ForecastDisagreementStrategy; charge_battery_extended_mode?: ChargeBatteryExtendedMode}): Knobs {
        return new Knobs(
            'force_disable_export' in overrides ? overrides.force_disable_export! : this._force_disable_export,
            'export_soc_threshold' in overrides ? overrides.export_soc_threshold! : this._export_soc_threshold,
            'discharge_soc_target' in overrides ? overrides.discharge_soc_target! : this._discharge_soc_target,
            'battery_soc_target' in overrides ? overrides.battery_soc_target! : this._battery_soc_target,
            'full_charge_discharge_soc_target' in overrides ? overrides.full_charge_discharge_soc_target! : this._full_charge_discharge_soc_target,
            'full_charge_export_soc_threshold' in overrides ? overrides.full_charge_export_soc_threshold! : this._full_charge_export_soc_threshold,
            'discharge_time' in overrides ? overrides.discharge_time! : this._discharge_time,
            'debug_full_charge' in overrides ? overrides.debug_full_charge! : this._debug_full_charge,
            'pessimism_multiplier_modifier' in overrides ? overrides.pessimism_multiplier_modifier! : this._pessimism_multiplier_modifier,
            'disable_night_grid_discharge' in overrides ? overrides.disable_night_grid_discharge! : this._disable_night_grid_discharge,
            'charge_car_boost' in overrides ? overrides.charge_car_boost! : this._charge_car_boost,
            'charge_car_extended' in overrides ? overrides.charge_car_extended! : this._charge_car_extended,
            'zappi_current_target' in overrides ? overrides.zappi_current_target! : this._zappi_current_target,
            'zappi_limit' in overrides ? overrides.zappi_limit! : this._zappi_limit,
            'zappi_emergency_margin' in overrides ? overrides.zappi_emergency_margin! : this._zappi_emergency_margin,
            'grid_export_limit_w' in overrides ? overrides.grid_export_limit_w! : this._grid_export_limit_w,
            'grid_import_limit_w' in overrides ? overrides.grid_import_limit_w! : this._grid_import_limit_w,
            'allow_battery_to_car' in overrides ? overrides.allow_battery_to_car! : this._allow_battery_to_car,
            'eddi_enable_soc' in overrides ? overrides.eddi_enable_soc! : this._eddi_enable_soc,
            'eddi_disable_soc' in overrides ? overrides.eddi_disable_soc! : this._eddi_disable_soc,
            'eddi_dwell_s' in overrides ? overrides.eddi_dwell_s! : this._eddi_dwell_s,
            'weathersoc_winter_temperature_threshold' in overrides ? overrides.weathersoc_winter_temperature_threshold! : this._weathersoc_winter_temperature_threshold,
            'weathersoc_low_energy_threshold' in overrides ? overrides.weathersoc_low_energy_threshold! : this._weathersoc_low_energy_threshold,
            'weathersoc_ok_energy_threshold' in overrides ? overrides.weathersoc_ok_energy_threshold! : this._weathersoc_ok_energy_threshold,
            'weathersoc_high_energy_threshold' in overrides ? overrides.weathersoc_high_energy_threshold! : this._weathersoc_high_energy_threshold,
            'weathersoc_too_much_energy_threshold' in overrides ? overrides.weathersoc_too_much_energy_threshold! : this._weathersoc_too_much_energy_threshold,
            'writes_enabled' in overrides ? overrides.writes_enabled! : this._writes_enabled,
            'forecast_disagreement_strategy' in overrides ? overrides.forecast_disagreement_strategy! : this._forecast_disagreement_strategy,
            'charge_battery_extended_mode' in overrides ? overrides.charge_battery_extended_mode! : this._charge_battery_extended_mode
        );
    }

    public static fromPlain(obj: {force_disable_export: boolean; export_soc_threshold: number; discharge_soc_target: number; battery_soc_target: number; full_charge_discharge_soc_target: number; full_charge_export_soc_threshold: number; discharge_time: DischargeTime; debug_full_charge: DebugFullCharge; pessimism_multiplier_modifier: number; disable_night_grid_discharge: boolean; charge_car_boost: boolean; charge_car_extended: boolean; zappi_current_target: number; zappi_limit: number; zappi_emergency_margin: number; grid_export_limit_w: number; grid_import_limit_w: number; allow_battery_to_car: boolean; eddi_enable_soc: number; eddi_disable_soc: number; eddi_dwell_s: number; weathersoc_winter_temperature_threshold: number; weathersoc_low_energy_threshold: number; weathersoc_ok_energy_threshold: number; weathersoc_high_energy_threshold: number; weathersoc_too_much_energy_threshold: number; writes_enabled: boolean; forecast_disagreement_strategy: ForecastDisagreementStrategy; charge_battery_extended_mode: ChargeBatteryExtendedMode}): Knobs {
        return new Knobs(
            obj.force_disable_export,
            obj.export_soc_threshold,
            obj.discharge_soc_target,
            obj.battery_soc_target,
            obj.full_charge_discharge_soc_target,
            obj.full_charge_export_soc_threshold,
            obj.discharge_time,
            obj.debug_full_charge,
            obj.pessimism_multiplier_modifier,
            obj.disable_night_grid_discharge,
            obj.charge_car_boost,
            obj.charge_car_extended,
            obj.zappi_current_target,
            obj.zappi_limit,
            obj.zappi_emergency_margin,
            obj.grid_export_limit_w,
            obj.grid_import_limit_w,
            obj.allow_battery_to_car,
            obj.eddi_enable_soc,
            obj.eddi_disable_soc,
            obj.eddi_dwell_s,
            obj.weathersoc_winter_temperature_threshold,
            obj.weathersoc_low_energy_threshold,
            obj.weathersoc_ok_energy_threshold,
            obj.weathersoc_high_energy_threshold,
            obj.weathersoc_too_much_energy_threshold,
            obj.writes_enabled,
            obj.forecast_disagreement_strategy,
            obj.charge_battery_extended_mode
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Knobs.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Knobs.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Knobs'
    public baboonTypeIdentifier() {
        return Knobs.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0"]
    public baboonSameInVersions() {
        return Knobs.BaboonSameInVersions
    }
    public static binCodec(): Knobs_UEBACodec {
        return Knobs_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class Knobs_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Knobs, writer: BaboonBinWriter): unknown {
        if (this !== Knobs_UEBACodec.lazyInstance.value) {
          return Knobs_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeBool(buffer, value.force_disable_export);
            BinTools.writeF64(buffer, value.export_soc_threshold);
            BinTools.writeF64(buffer, value.discharge_soc_target);
            BinTools.writeF64(buffer, value.battery_soc_target);
            BinTools.writeF64(buffer, value.full_charge_discharge_soc_target);
            BinTools.writeF64(buffer, value.full_charge_export_soc_threshold);
            DischargeTime_UEBACodec.instance.encode(ctx, value.discharge_time, buffer);
            DebugFullCharge_UEBACodec.instance.encode(ctx, value.debug_full_charge, buffer);
            BinTools.writeF64(buffer, value.pessimism_multiplier_modifier);
            BinTools.writeBool(buffer, value.disable_night_grid_discharge);
            BinTools.writeBool(buffer, value.charge_car_boost);
            BinTools.writeBool(buffer, value.charge_car_extended);
            BinTools.writeF64(buffer, value.zappi_current_target);
            BinTools.writeF64(buffer, value.zappi_limit);
            BinTools.writeF64(buffer, value.zappi_emergency_margin);
            BinTools.writeI32(buffer, value.grid_export_limit_w);
            BinTools.writeI32(buffer, value.grid_import_limit_w);
            BinTools.writeBool(buffer, value.allow_battery_to_car);
            BinTools.writeF64(buffer, value.eddi_enable_soc);
            BinTools.writeF64(buffer, value.eddi_disable_soc);
            BinTools.writeI32(buffer, value.eddi_dwell_s);
            BinTools.writeF64(buffer, value.weathersoc_winter_temperature_threshold);
            BinTools.writeF64(buffer, value.weathersoc_low_energy_threshold);
            BinTools.writeF64(buffer, value.weathersoc_ok_energy_threshold);
            BinTools.writeF64(buffer, value.weathersoc_high_energy_threshold);
            BinTools.writeF64(buffer, value.weathersoc_too_much_energy_threshold);
            BinTools.writeBool(buffer, value.writes_enabled);
            ForecastDisagreementStrategy_UEBACodec.instance.encode(ctx, value.forecast_disagreement_strategy, buffer);
            ChargeBatteryExtendedMode_UEBACodec.instance.encode(ctx, value.charge_battery_extended_mode, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeBool(writer, value.force_disable_export);
            BinTools.writeF64(writer, value.export_soc_threshold);
            BinTools.writeF64(writer, value.discharge_soc_target);
            BinTools.writeF64(writer, value.battery_soc_target);
            BinTools.writeF64(writer, value.full_charge_discharge_soc_target);
            BinTools.writeF64(writer, value.full_charge_export_soc_threshold);
            DischargeTime_UEBACodec.instance.encode(ctx, value.discharge_time, writer);
            DebugFullCharge_UEBACodec.instance.encode(ctx, value.debug_full_charge, writer);
            BinTools.writeF64(writer, value.pessimism_multiplier_modifier);
            BinTools.writeBool(writer, value.disable_night_grid_discharge);
            BinTools.writeBool(writer, value.charge_car_boost);
            BinTools.writeBool(writer, value.charge_car_extended);
            BinTools.writeF64(writer, value.zappi_current_target);
            BinTools.writeF64(writer, value.zappi_limit);
            BinTools.writeF64(writer, value.zappi_emergency_margin);
            BinTools.writeI32(writer, value.grid_export_limit_w);
            BinTools.writeI32(writer, value.grid_import_limit_w);
            BinTools.writeBool(writer, value.allow_battery_to_car);
            BinTools.writeF64(writer, value.eddi_enable_soc);
            BinTools.writeF64(writer, value.eddi_disable_soc);
            BinTools.writeI32(writer, value.eddi_dwell_s);
            BinTools.writeF64(writer, value.weathersoc_winter_temperature_threshold);
            BinTools.writeF64(writer, value.weathersoc_low_energy_threshold);
            BinTools.writeF64(writer, value.weathersoc_ok_energy_threshold);
            BinTools.writeF64(writer, value.weathersoc_high_energy_threshold);
            BinTools.writeF64(writer, value.weathersoc_too_much_energy_threshold);
            BinTools.writeBool(writer, value.writes_enabled);
            ForecastDisagreementStrategy_UEBACodec.instance.encode(ctx, value.forecast_disagreement_strategy, writer);
            ChargeBatteryExtendedMode_UEBACodec.instance.encode(ctx, value.charge_battery_extended_mode, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Knobs {
        if (this !== Knobs_UEBACodec .lazyInstance.value) {
            return Knobs_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const force_disable_export = BinTools.readBool(reader);
        const export_soc_threshold = BinTools.readF64(reader);
        const discharge_soc_target = BinTools.readF64(reader);
        const battery_soc_target = BinTools.readF64(reader);
        const full_charge_discharge_soc_target = BinTools.readF64(reader);
        const full_charge_export_soc_threshold = BinTools.readF64(reader);
        const discharge_time = DischargeTime_UEBACodec.instance.decode(ctx, reader);
        const debug_full_charge = DebugFullCharge_UEBACodec.instance.decode(ctx, reader);
        const pessimism_multiplier_modifier = BinTools.readF64(reader);
        const disable_night_grid_discharge = BinTools.readBool(reader);
        const charge_car_boost = BinTools.readBool(reader);
        const charge_car_extended = BinTools.readBool(reader);
        const zappi_current_target = BinTools.readF64(reader);
        const zappi_limit = BinTools.readF64(reader);
        const zappi_emergency_margin = BinTools.readF64(reader);
        const grid_export_limit_w = BinTools.readI32(reader);
        const grid_import_limit_w = BinTools.readI32(reader);
        const allow_battery_to_car = BinTools.readBool(reader);
        const eddi_enable_soc = BinTools.readF64(reader);
        const eddi_disable_soc = BinTools.readF64(reader);
        const eddi_dwell_s = BinTools.readI32(reader);
        const weathersoc_winter_temperature_threshold = BinTools.readF64(reader);
        const weathersoc_low_energy_threshold = BinTools.readF64(reader);
        const weathersoc_ok_energy_threshold = BinTools.readF64(reader);
        const weathersoc_high_energy_threshold = BinTools.readF64(reader);
        const weathersoc_too_much_energy_threshold = BinTools.readF64(reader);
        const writes_enabled = BinTools.readBool(reader);
        const forecast_disagreement_strategy = ForecastDisagreementStrategy_UEBACodec.instance.decode(ctx, reader);
        const charge_battery_extended_mode = ChargeBatteryExtendedMode_UEBACodec.instance.decode(ctx, reader);
        return new Knobs(
            force_disable_export,
            export_soc_threshold,
            discharge_soc_target,
            battery_soc_target,
            full_charge_discharge_soc_target,
            full_charge_export_soc_threshold,
            discharge_time,
            debug_full_charge,
            pessimism_multiplier_modifier,
            disable_night_grid_discharge,
            charge_car_boost,
            charge_car_extended,
            zappi_current_target,
            zappi_limit,
            zappi_emergency_margin,
            grid_export_limit_w,
            grid_import_limit_w,
            allow_battery_to_car,
            eddi_enable_soc,
            eddi_disable_soc,
            eddi_dwell_s,
            weathersoc_winter_temperature_threshold,
            weathersoc_low_energy_threshold,
            weathersoc_ok_energy_threshold,
            weathersoc_high_energy_threshold,
            weathersoc_too_much_energy_threshold,
            writes_enabled,
            forecast_disagreement_strategy,
            charge_battery_extended_mode,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Knobs_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Knobs_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Knobs'
    public baboonTypeIdentifier() {
        return Knobs_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Knobs_UEBACodec())
    public static get instance(): Knobs_UEBACodec {
        return Knobs_UEBACodec.lazyInstance.value
    }
}