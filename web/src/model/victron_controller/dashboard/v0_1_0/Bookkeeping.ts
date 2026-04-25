// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export class Bookkeeping implements BaboonGenerated {
    private readonly _next_full_charge_iso: string | undefined;
    private readonly _above_soc_date_iso: string | undefined;
    private readonly _prev_ess_state: number | undefined;
    private readonly _zappi_active: boolean;
    private readonly _charge_to_full_required: boolean;
    private readonly _soc_end_of_day_target: number;
    private readonly _effective_export_soc_threshold: number;
    private readonly _battery_selected_soc_target: number;
    private readonly _charge_battery_extended_today: boolean;
    private readonly _charge_battery_extended_today_date_iso: string | undefined;

    constructor(next_full_charge_iso: string | undefined, above_soc_date_iso: string | undefined, prev_ess_state: number | undefined, zappi_active: boolean, charge_to_full_required: boolean, soc_end_of_day_target: number, effective_export_soc_threshold: number, battery_selected_soc_target: number, charge_battery_extended_today: boolean, charge_battery_extended_today_date_iso: string | undefined) {
        this._next_full_charge_iso = next_full_charge_iso
        this._above_soc_date_iso = above_soc_date_iso
        this._prev_ess_state = prev_ess_state
        this._zappi_active = zappi_active
        this._charge_to_full_required = charge_to_full_required
        this._soc_end_of_day_target = soc_end_of_day_target
        this._effective_export_soc_threshold = effective_export_soc_threshold
        this._battery_selected_soc_target = battery_selected_soc_target
        this._charge_battery_extended_today = charge_battery_extended_today
        this._charge_battery_extended_today_date_iso = charge_battery_extended_today_date_iso
    }

    public get next_full_charge_iso(): string | undefined {
        return this._next_full_charge_iso;
    }
    public get above_soc_date_iso(): string | undefined {
        return this._above_soc_date_iso;
    }
    public get prev_ess_state(): number | undefined {
        return this._prev_ess_state;
    }
    public get zappi_active(): boolean {
        return this._zappi_active;
    }
    public get charge_to_full_required(): boolean {
        return this._charge_to_full_required;
    }
    public get soc_end_of_day_target(): number {
        return this._soc_end_of_day_target;
    }
    public get effective_export_soc_threshold(): number {
        return this._effective_export_soc_threshold;
    }
    public get battery_selected_soc_target(): number {
        return this._battery_selected_soc_target;
    }
    public get charge_battery_extended_today(): boolean {
        return this._charge_battery_extended_today;
    }
    public get charge_battery_extended_today_date_iso(): string | undefined {
        return this._charge_battery_extended_today_date_iso;
    }

    public toJSON(): Record<string, unknown> {
        return {
            next_full_charge_iso: this._next_full_charge_iso !== undefined ? this._next_full_charge_iso : undefined,
            above_soc_date_iso: this._above_soc_date_iso !== undefined ? this._above_soc_date_iso : undefined,
            prev_ess_state: this._prev_ess_state !== undefined ? this._prev_ess_state : undefined,
            zappi_active: this._zappi_active,
            charge_to_full_required: this._charge_to_full_required,
            soc_end_of_day_target: this._soc_end_of_day_target,
            effective_export_soc_threshold: this._effective_export_soc_threshold,
            battery_selected_soc_target: this._battery_selected_soc_target,
            charge_battery_extended_today: this._charge_battery_extended_today,
            charge_battery_extended_today_date_iso: this._charge_battery_extended_today_date_iso !== undefined ? this._charge_battery_extended_today_date_iso : undefined
        };
    }

    public with(overrides: {next_full_charge_iso?: string | undefined; above_soc_date_iso?: string | undefined; prev_ess_state?: number | undefined; zappi_active?: boolean; charge_to_full_required?: boolean; soc_end_of_day_target?: number; effective_export_soc_threshold?: number; battery_selected_soc_target?: number; charge_battery_extended_today?: boolean; charge_battery_extended_today_date_iso?: string | undefined}): Bookkeeping {
        return new Bookkeeping(
            'next_full_charge_iso' in overrides ? overrides.next_full_charge_iso! : this._next_full_charge_iso,
            'above_soc_date_iso' in overrides ? overrides.above_soc_date_iso! : this._above_soc_date_iso,
            'prev_ess_state' in overrides ? overrides.prev_ess_state! : this._prev_ess_state,
            'zappi_active' in overrides ? overrides.zappi_active! : this._zappi_active,
            'charge_to_full_required' in overrides ? overrides.charge_to_full_required! : this._charge_to_full_required,
            'soc_end_of_day_target' in overrides ? overrides.soc_end_of_day_target! : this._soc_end_of_day_target,
            'effective_export_soc_threshold' in overrides ? overrides.effective_export_soc_threshold! : this._effective_export_soc_threshold,
            'battery_selected_soc_target' in overrides ? overrides.battery_selected_soc_target! : this._battery_selected_soc_target,
            'charge_battery_extended_today' in overrides ? overrides.charge_battery_extended_today! : this._charge_battery_extended_today,
            'charge_battery_extended_today_date_iso' in overrides ? overrides.charge_battery_extended_today_date_iso! : this._charge_battery_extended_today_date_iso
        );
    }

    public static fromPlain(obj: {next_full_charge_iso: string | undefined; above_soc_date_iso: string | undefined; prev_ess_state: number | undefined; zappi_active: boolean; charge_to_full_required: boolean; soc_end_of_day_target: number; effective_export_soc_threshold: number; battery_selected_soc_target: number; charge_battery_extended_today: boolean; charge_battery_extended_today_date_iso: string | undefined}): Bookkeeping {
        return new Bookkeeping(
            obj.next_full_charge_iso,
            obj.above_soc_date_iso,
            obj.prev_ess_state,
            obj.zappi_active,
            obj.charge_to_full_required,
            obj.soc_end_of_day_target,
            obj.effective_export_soc_threshold,
            obj.battery_selected_soc_target,
            obj.charge_battery_extended_today,
            obj.charge_battery_extended_today_date_iso
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Bookkeeping.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Bookkeeping.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Bookkeeping'
    public baboonTypeIdentifier() {
        return Bookkeeping.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0"]
    public baboonSameInVersions() {
        return Bookkeeping.BaboonSameInVersions
    }
    public static binCodec(): Bookkeeping_UEBACodec {
        return Bookkeeping_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class Bookkeeping_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Bookkeeping, writer: BaboonBinWriter): unknown {
        if (this !== Bookkeeping_UEBACodec.lazyInstance.value) {
          return Bookkeeping_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.next_full_charge_iso === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.next_full_charge_iso);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.above_soc_date_iso === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.above_soc_date_iso);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.prev_ess_state === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeI32(buffer, value.prev_ess_state);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeBool(buffer, value.zappi_active);
            BinTools.writeBool(buffer, value.charge_to_full_required);
            BinTools.writeF64(buffer, value.soc_end_of_day_target);
            BinTools.writeF64(buffer, value.effective_export_soc_threshold);
            BinTools.writeF64(buffer, value.battery_selected_soc_target);
            BinTools.writeBool(buffer, value.charge_battery_extended_today);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.charge_battery_extended_today_date_iso === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.charge_battery_extended_today_date_iso);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.next_full_charge_iso === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.next_full_charge_iso);
            }
            if (value.above_soc_date_iso === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.above_soc_date_iso);
            }
            if (value.prev_ess_state === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeI32(writer, value.prev_ess_state);
            }
            BinTools.writeBool(writer, value.zappi_active);
            BinTools.writeBool(writer, value.charge_to_full_required);
            BinTools.writeF64(writer, value.soc_end_of_day_target);
            BinTools.writeF64(writer, value.effective_export_soc_threshold);
            BinTools.writeF64(writer, value.battery_selected_soc_target);
            BinTools.writeBool(writer, value.charge_battery_extended_today);
            if (value.charge_battery_extended_today_date_iso === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.charge_battery_extended_today_date_iso);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Bookkeeping {
        if (this !== Bookkeeping_UEBACodec .lazyInstance.value) {
            return Bookkeeping_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 4; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const next_full_charge_iso = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const above_soc_date_iso = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const prev_ess_state = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readI32(reader));
        const zappi_active = BinTools.readBool(reader);
        const charge_to_full_required = BinTools.readBool(reader);
        const soc_end_of_day_target = BinTools.readF64(reader);
        const effective_export_soc_threshold = BinTools.readF64(reader);
        const battery_selected_soc_target = BinTools.readF64(reader);
        const charge_battery_extended_today = BinTools.readBool(reader);
        const charge_battery_extended_today_date_iso = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        return new Bookkeeping(
            next_full_charge_iso,
            above_soc_date_iso,
            prev_ess_state,
            zappi_active,
            charge_to_full_required,
            soc_end_of_day_target,
            effective_export_soc_threshold,
            battery_selected_soc_target,
            charge_battery_extended_today,
            charge_battery_extended_today_date_iso,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Bookkeeping_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Bookkeeping_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Bookkeeping'
    public baboonTypeIdentifier() {
        return Bookkeeping_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Bookkeeping_UEBACodec())
    public static get instance(): Bookkeeping_UEBACodec {
        return Bookkeeping_UEBACodec.lazyInstance.value
    }
}