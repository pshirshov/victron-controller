// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class Diagnostics implements BaboonGeneratedLatest {
    private readonly _process_uptime_s: bigint;
    private readonly _process_rss_bytes: bigint;
    private readonly _process_vm_hwm_bytes: bigint;
    private readonly _process_vm_size_bytes: bigint;
    private readonly _jemalloc_allocated_bytes: bigint;
    private readonly _jemalloc_resident_bytes: bigint;
    private readonly _host_mem_total_bytes: bigint;
    private readonly _host_mem_available_bytes: bigint;
    private readonly _host_swap_used_bytes: bigint;
    private readonly _sampled_at_epoch_ms: bigint;

    constructor(process_uptime_s: bigint, process_rss_bytes: bigint, process_vm_hwm_bytes: bigint, process_vm_size_bytes: bigint, jemalloc_allocated_bytes: bigint, jemalloc_resident_bytes: bigint, host_mem_total_bytes: bigint, host_mem_available_bytes: bigint, host_swap_used_bytes: bigint, sampled_at_epoch_ms: bigint) {
        this._process_uptime_s = process_uptime_s
        this._process_rss_bytes = process_rss_bytes
        this._process_vm_hwm_bytes = process_vm_hwm_bytes
        this._process_vm_size_bytes = process_vm_size_bytes
        this._jemalloc_allocated_bytes = jemalloc_allocated_bytes
        this._jemalloc_resident_bytes = jemalloc_resident_bytes
        this._host_mem_total_bytes = host_mem_total_bytes
        this._host_mem_available_bytes = host_mem_available_bytes
        this._host_swap_used_bytes = host_swap_used_bytes
        this._sampled_at_epoch_ms = sampled_at_epoch_ms
    }

    public get process_uptime_s(): bigint {
        return this._process_uptime_s;
    }
    public get process_rss_bytes(): bigint {
        return this._process_rss_bytes;
    }
    public get process_vm_hwm_bytes(): bigint {
        return this._process_vm_hwm_bytes;
    }
    public get process_vm_size_bytes(): bigint {
        return this._process_vm_size_bytes;
    }
    public get jemalloc_allocated_bytes(): bigint {
        return this._jemalloc_allocated_bytes;
    }
    public get jemalloc_resident_bytes(): bigint {
        return this._jemalloc_resident_bytes;
    }
    public get host_mem_total_bytes(): bigint {
        return this._host_mem_total_bytes;
    }
    public get host_mem_available_bytes(): bigint {
        return this._host_mem_available_bytes;
    }
    public get host_swap_used_bytes(): bigint {
        return this._host_swap_used_bytes;
    }
    public get sampled_at_epoch_ms(): bigint {
        return this._sampled_at_epoch_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            process_uptime_s: this._process_uptime_s,
            process_rss_bytes: this._process_rss_bytes,
            process_vm_hwm_bytes: this._process_vm_hwm_bytes,
            process_vm_size_bytes: this._process_vm_size_bytes,
            jemalloc_allocated_bytes: this._jemalloc_allocated_bytes,
            jemalloc_resident_bytes: this._jemalloc_resident_bytes,
            host_mem_total_bytes: this._host_mem_total_bytes,
            host_mem_available_bytes: this._host_mem_available_bytes,
            host_swap_used_bytes: this._host_swap_used_bytes,
            sampled_at_epoch_ms: this._sampled_at_epoch_ms
        };
    }

    public with(overrides: {process_uptime_s?: bigint; process_rss_bytes?: bigint; process_vm_hwm_bytes?: bigint; process_vm_size_bytes?: bigint; jemalloc_allocated_bytes?: bigint; jemalloc_resident_bytes?: bigint; host_mem_total_bytes?: bigint; host_mem_available_bytes?: bigint; host_swap_used_bytes?: bigint; sampled_at_epoch_ms?: bigint}): Diagnostics {
        return new Diagnostics(
            'process_uptime_s' in overrides ? overrides.process_uptime_s! : this._process_uptime_s,
            'process_rss_bytes' in overrides ? overrides.process_rss_bytes! : this._process_rss_bytes,
            'process_vm_hwm_bytes' in overrides ? overrides.process_vm_hwm_bytes! : this._process_vm_hwm_bytes,
            'process_vm_size_bytes' in overrides ? overrides.process_vm_size_bytes! : this._process_vm_size_bytes,
            'jemalloc_allocated_bytes' in overrides ? overrides.jemalloc_allocated_bytes! : this._jemalloc_allocated_bytes,
            'jemalloc_resident_bytes' in overrides ? overrides.jemalloc_resident_bytes! : this._jemalloc_resident_bytes,
            'host_mem_total_bytes' in overrides ? overrides.host_mem_total_bytes! : this._host_mem_total_bytes,
            'host_mem_available_bytes' in overrides ? overrides.host_mem_available_bytes! : this._host_mem_available_bytes,
            'host_swap_used_bytes' in overrides ? overrides.host_swap_used_bytes! : this._host_swap_used_bytes,
            'sampled_at_epoch_ms' in overrides ? overrides.sampled_at_epoch_ms! : this._sampled_at_epoch_ms
        );
    }

    public static fromPlain(obj: {process_uptime_s: bigint; process_rss_bytes: bigint; process_vm_hwm_bytes: bigint; process_vm_size_bytes: bigint; jemalloc_allocated_bytes: bigint; jemalloc_resident_bytes: bigint; host_mem_total_bytes: bigint; host_mem_available_bytes: bigint; host_swap_used_bytes: bigint; sampled_at_epoch_ms: bigint}): Diagnostics {
        return new Diagnostics(
            obj.process_uptime_s,
            obj.process_rss_bytes,
            obj.process_vm_hwm_bytes,
            obj.process_vm_size_bytes,
            obj.jemalloc_allocated_bytes,
            obj.jemalloc_resident_bytes,
            obj.host_mem_total_bytes,
            obj.host_mem_available_bytes,
            obj.host_swap_used_bytes,
            obj.sampled_at_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Diagnostics.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Diagnostics.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Diagnostics'
    public baboonTypeIdentifier() {
        return Diagnostics.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return Diagnostics.BaboonSameInVersions
    }
    public static binCodec(): Diagnostics_UEBACodec {
        return Diagnostics_UEBACodec.instance
    }
}

export class Diagnostics_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Diagnostics, writer: BaboonBinWriter): unknown {
        if (this !== Diagnostics_UEBACodec.lazyInstance.value) {
          return Diagnostics_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI64(buffer, value.process_uptime_s);
            BinTools.writeI64(buffer, value.process_rss_bytes);
            BinTools.writeI64(buffer, value.process_vm_hwm_bytes);
            BinTools.writeI64(buffer, value.process_vm_size_bytes);
            BinTools.writeI64(buffer, value.jemalloc_allocated_bytes);
            BinTools.writeI64(buffer, value.jemalloc_resident_bytes);
            BinTools.writeI64(buffer, value.host_mem_total_bytes);
            BinTools.writeI64(buffer, value.host_mem_available_bytes);
            BinTools.writeI64(buffer, value.host_swap_used_bytes);
            BinTools.writeI64(buffer, value.sampled_at_epoch_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI64(writer, value.process_uptime_s);
            BinTools.writeI64(writer, value.process_rss_bytes);
            BinTools.writeI64(writer, value.process_vm_hwm_bytes);
            BinTools.writeI64(writer, value.process_vm_size_bytes);
            BinTools.writeI64(writer, value.jemalloc_allocated_bytes);
            BinTools.writeI64(writer, value.jemalloc_resident_bytes);
            BinTools.writeI64(writer, value.host_mem_total_bytes);
            BinTools.writeI64(writer, value.host_mem_available_bytes);
            BinTools.writeI64(writer, value.host_swap_used_bytes);
            BinTools.writeI64(writer, value.sampled_at_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Diagnostics {
        if (this !== Diagnostics_UEBACodec .lazyInstance.value) {
            return Diagnostics_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const process_uptime_s = BinTools.readI64(reader);
        const process_rss_bytes = BinTools.readI64(reader);
        const process_vm_hwm_bytes = BinTools.readI64(reader);
        const process_vm_size_bytes = BinTools.readI64(reader);
        const jemalloc_allocated_bytes = BinTools.readI64(reader);
        const jemalloc_resident_bytes = BinTools.readI64(reader);
        const host_mem_total_bytes = BinTools.readI64(reader);
        const host_mem_available_bytes = BinTools.readI64(reader);
        const host_swap_used_bytes = BinTools.readI64(reader);
        const sampled_at_epoch_ms = BinTools.readI64(reader);
        return new Diagnostics(
            process_uptime_s,
            process_rss_bytes,
            process_vm_hwm_bytes,
            process_vm_size_bytes,
            jemalloc_allocated_bytes,
            jemalloc_resident_bytes,
            host_mem_total_bytes,
            host_mem_available_bytes,
            host_swap_used_bytes,
            sampled_at_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Diagnostics_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Diagnostics_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Diagnostics'
    public baboonTypeIdentifier() {
        return Diagnostics_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Diagnostics_UEBACodec())
    public static get instance(): Diagnostics_UEBACodec {
        return Diagnostics_UEBACodec.lazyInstance.value
    }
}